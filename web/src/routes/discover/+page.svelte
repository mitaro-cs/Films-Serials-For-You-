<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, errorText, post } from '$lib/api';
  import Poster from '$lib/discover/Poster.svelte';
  import PosterRow from '$lib/discover/PosterRow.svelte';
  import { extSheet } from '$lib/discover/sheet.svelte';
  import LinkFinder from '$lib/links/LinkFinder.svelte';
  import { session } from '$lib/session.svelte';
  import type { ExtCard } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  // Обзор как на IMDb: популярное и топ-250 (не только то, что есть у нас), поиск по всему IMDb,
  // видео по ссылке. Карточка открывается поверх страницы (ExtSheet).
  type Row = { key: string; title: string; items: ExtCard[] };

  const q = $derived(page.url.searchParams.get('q') ?? '');
  const chart = $derived(page.url.searchParams.get('chart'));

  let feed = $state<{ rows: Row[]; updatedAt: number | null } | null>(null);
  let full = $state<Row | null>(null);
  let found = $state<ExtCard[] | null>(null);
  let failed = $state('');
  let text = $state('');
  let finder = $state(false);
  let searching = $state(false);

  $effect(() => {
    text = q;
  });

  // Пустая лента — сервер только начал загрузку: спрашиваем ещё раз, пока не появится.
  $effect(() => {
    void extSheet.version;
    if (chart) return;
    let timer = 0;
    let tries = 0;
    const load = () =>
      api<{ rows: Row[]; updatedAt: number | null }>('/api/discover').then(
        (r) => {
          feed = r;
          failed = '';
          if (!r.rows.length && ++tries < 10) timer = window.setTimeout(load, 4000);
        },
        (e) => (failed = errorText(e)),
      );
    load();
    return () => clearTimeout(timer);
  });

  $effect(() => {
    void extSheet.version;
    if (!chart) {
      full = null;
      return;
    }
    api<Row>(`/api/discover/chart/${encodeURIComponent(chart)}`).then(
      (r) => (full = r),
      (e) => (failed = errorText(e)),
    );
  });

  $effect(() => {
    void extSheet.version;
    const query = q.trim();
    if (query.length < 2) {
      found = null;
      return;
    }
    searching = true;
    api<ExtCard[]>(`/api/discover/search?q=${encodeURIComponent(query)}`)
      .then(
        (r) => (found = r),
        (e) => {
          found = [];
          toast(errorText(e));
        },
      )
      .finally(() => (searching = false));
  });

  function submit(e: SubmitEvent) {
    e.preventDefault();
    const v = text.trim();
    goto(v ? `/discover?q=${encodeURIComponent(v)}` : '/discover', { keepFocus: true, noScroll: true });
  }

  async function refresh() {
    try {
      await post('/api/admin/discover/refresh');
      toast('Лента обновится через минуту');
    } catch (e) {
      toast(errorText(e));
    }
  }
</script>

<div class="page">
  <div class="top">
    <form class="find" role="search" onsubmit={submit}>
      <Icon name="search" size={20} />
      <input type="search" bind:value={text} placeholder="Поиск по всему IMDb" aria-label="Поиск по IMDb" />
      {#if searching}<span class="spin" aria-hidden="true"></span>{/if}
    </form>
    <button class="btn" onclick={() => (finder = true)}><Icon name="link" size={18} /> Смотреть по ссылке</button>
  </div>

  {#if q.trim().length >= 2}
    <section aria-label="Найдено">
      <div class="head">
        <h2>Найдено на IMDb</h2>
        <a class="muted" href="/discover">Сбросить</a>
      </div>
      {#if found === null}
        <div class="grid">{#each Array(12) as _, i (i)}<span class="ph skeleton"></span>{/each}</div>
      {:else if found.length}
        <div class="grid">{#each found as card, i (card.id)}<Poster {card} {i} />{/each}</div>
      {:else}
        <div class="empty"><Icon name="search" size={40} /><p>По запросу «{q}» ничего нет</p></div>
      {/if}
    </section>
  {:else if chart}
    <section aria-label={full?.title ?? 'Подборка'}>
      <div class="head">
        <h2>{full?.title ?? ''}</h2>
        <a class="muted" href="/discover"><Icon name="back" size={14} /> Обзор</a>
      </div>
      {#if full}
        <div class="grid">{#each full.items as card, i (card.id)}<Poster {card} rank={chart.startsWith('top') ? i + 1 : null} {i} />{/each}</div>
      {:else}
        <div class="grid">{#each Array(18) as _, i (i)}<span class="ph skeleton"></span>{/each}</div>
      {/if}
    </section>
  {:else if failed && !feed}
    <div class="empty tall"><Icon name="offline" size={40} /><p>{failed}</p></div>
  {:else if feed === null}
    {#each Array(2) as _, r (r)}
      <div class="ph-row">
        <span class="ph-title skeleton"></span>
        <div class="ph-cards">{#each Array(8) as _, i (i)}<span class="ph skeleton"></span>{/each}</div>
      </div>
    {/each}
  {:else if feed.rows.length}
    {#each feed.rows as row, i (row.key)}
      <PosterRow title={row.title} items={row.items} href="/discover?chart={row.key}" ranked={row.key.startsWith('top')} {i} />
    {/each}
    {#if session.me?.manage}
      <p class="foot muted">
        Данные IMDb{#if feed.updatedAt}, обновлено {new Date(feed.updatedAt).toLocaleString('ru-RU', { day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' })}{/if}.
        <button class="linkish" onclick={refresh}>Обновить</button>
      </p>
    {/if}
  {:else}
    <div class="empty tall">
      <Icon name="globe" size={40} />
      <p>Загружаем подборки IMDb — это займёт минуту.</p>
      <p class="muted">Пока можно поискать фильм выше.</p>
    </div>
  {/if}
</div>

<LinkFinder bind:open={finder} />

<style>
  .top {
    display: flex;
    gap: 10px;
    margin-bottom: 28px;
  }
  .find {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 48px;
    padding: 0 16px;
    border-radius: 14px;
    background: var(--surface-2);
    color: var(--muted);
  }
  .find:focus-within {
    box-shadow: inset 0 0 0 1px var(--accent-2);
  }
  .find input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: none;
    font-size: 16px;
    color: var(--text);
  }
  .top .btn {
    height: 48px;
    flex: none;
  }
  @media (max-width: 640px) {
    .top {
      flex-direction: column;
    }
    .find {
      flex: none;
    }
  }
  .spin {
    width: 16px;
    height: 16px;
    border: 2px solid var(--line);
    border-top-color: var(--text);
    border-radius: 50%;
    animation: turn 0.8s linear infinite;
  }
  @keyframes turn {
    to {
      transform: rotate(1turn);
    }
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 16px;
  }
  .head a {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 14px;
  }
  h2 {
    font-size: clamp(19px, 1.6vw, 24px);
    font-weight: 700;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 150px), 1fr));
    gap: 22px 16px;
  }
  @media (max-width: 640px) {
    .grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 16px 10px;
    }
  }
  .ph {
    aspect-ratio: 2 / 3;
    border-radius: 12px;
  }
  .ph-row {
    margin-bottom: 30px;
  }
  .ph-title {
    display: block;
    width: 180px;
    height: 24px;
    border-radius: 6px;
    margin-bottom: 14px;
  }
  .ph-cards {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: calc((100% - 6.4 * 16px) / 7.4);
    gap: 16px;
    overflow: hidden;
  }
  @media (max-width: 640px) {
    .ph-cards {
      grid-auto-columns: calc((100% - 1.6 * 10px) / 2.6);
      gap: 10px;
    }
  }
  .tall {
    min-height: 50vh;
  }
  .foot {
    font-size: 13px;
  }
  .linkish {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
</style>
