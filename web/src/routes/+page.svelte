<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api } from '$lib/api';
  import { counts, filter, left, percent, STATUSES, statusOf, tabOf, TITLES, type Status } from '$lib/library';
  import { session } from '$lib/session.svelte';
  import type { Card, Library } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import Ticket from '$lib/ui/Ticket.svelte';

  // Главная — каталог-билеты по макету: крупный заголовок раздела, фильтры-капсулы,
  // «Продолжить» (где остановились) и сетка карточек. Фильтры и поиск — на месте, без запросов.
  const tab = $derived(tabOf(page.url.searchParams));
  const status = $derived(statusOf(page.url.searchParams));

  let items = $state<Card[] | null>(null);
  let recent = $state<number[]>([]);
  let failed = $state(false);
  let query = $state('');
  let searching = $state(false);
  let input = $state<HTMLInputElement>();

  $effect(() => {
    const t = tab;
    items = null;
    failed = false;
    api<Library>(`/api/library?tab=${t}`).then(
      (r) => {
        if (t !== tab) return;
        items = r.items;
        recent = r.recent;
      },
      () => (failed = true),
    );
  });

  const shown = $derived(items ? filter(items, status, query) : []);
  const n = $derived(items ? counts(items) : null);
  const resume = $derived(
    status === 'all' && !query && items
      ? recent.map((id) => items!.find((c) => c.id === id && c.status === 'watching' && c.resume)).filter((c) => !!c)
      : [],
  );

  // Выбранный фильтр всегда виден, даже если лента капсул прокручена (узкий телефон).
  let chips = $state<HTMLDivElement>();
  $effect(() => {
    void status;
    chips?.querySelector<HTMLElement>('.on')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  });

  function setStatus(s: Status) {
    const u = new URL(page.url);
    if (s === 'all') u.searchParams.delete('status');
    else u.searchParams.set('status', s);
    goto(u, { replaceState: true, noScroll: true, keepFocus: true });
  }

  function toggleSearch() {
    searching = !searching;
    if (searching) queueMicrotask(() => input?.focus());
    else query = '';
  }

  const EMPTY: Record<Status, string> = {
    all: 'Здесь пока пусто',
    want: 'Отметьте «Хочу посмотреть» кнопкой на карточке',
    watching: 'Начатого нет — всё досмотрено',
    watched: 'Досмотренное появится здесь белыми билетами',
  };
</script>

<div class="page">
  <h1>{TITLES[tab]}</h1>

  <div class="filters">
    <div class="chips" role="tablist" aria-label="Статус" bind:this={chips}>
      {#each STATUSES as s (s.id)}
        <button class="chip" class:on={status === s.id} role="tab" aria-selected={status === s.id} onclick={() => setStatus(s.id)}>
          {s.label}
          {#if n && n[s.id]}<small>{n[s.id]}</small>{/if}
        </button>
      {/each}
    </div>
    <div class="find" class:open={searching}>
      {#if searching}
        <input bind:this={input} bind:value={query} type="search" placeholder="Название, режиссёр, год" aria-label="Найти в разделе" onkeydown={(e) => e.key === 'Escape' && toggleSearch()} />
      {/if}
      <button class="round" aria-label={searching ? 'Закрыть поиск' : 'Найти в разделе'} onclick={toggleSearch}>
        <Icon name={searching ? 'close' : 'search'} size={19} />
      </button>
    </div>
  </div>

  {#if resume.length}
    <section class="resume" aria-label="Продолжить просмотр">
      <h2>Продолжить</h2>
      <div class="strip">
        {#each resume as c (c.id)}
          <a class="go" href="/watch/{c.resume!.fileId}" style:--tint={c.accent ?? '#2c2c2e'}>
            {#if c.backdrop || c.poster}<img src={c.backdrop ?? c.poster} alt="" loading="lazy" decoding="async" />{/if}
            <span class="play"><Icon name="play" size={18} /></span>
            <span class="txt">
              <b>{c.title}</b>
              <small>{c.resume!.label || left(c.resume!.position, c.resume!.duration)}</small>
              {#if c.resume!.label}<small>{left(c.resume!.position, c.resume!.duration)}</small>{/if}
            </span>
            <span class="bar"><i style:width="{percent(c.resume!.position, c.resume!.duration)}%"></i></span>
          </a>
        {/each}
      </div>
    </section>
  {/if}

  {#if failed}
    <div class="empty"><Icon name="offline" size={40} /> Не удалось загрузить — проверьте связь</div>
  {:else if items === null}
    <div class="grid" aria-busy="true">
      {#each Array(8) as _, i (i)}<div class="ph skeleton"></div>{/each}
    </div>
  {:else if shown.length}
    <div class="grid">
      {#each shown as card, i (card.id)}<Ticket {card} eager={i < 8} />{/each}
    </div>
  {:else}
    <div class="empty">
      <Icon name={query ? 'search' : 'film'} size={40} />
      {#if query}
        Ничего не нашлось по «{query}»
      {:else if items.length === 0 && session.me?.manage}
        Фильмов ещё нет. <a class="btn primary sm" href="/admin?tab=folders">Добавить папку с фильмами</a>
      {:else}
        {EMPTY[status]}
      {/if}
    </div>
  {/if}
</div>

<style>
  h1 {
    font-size: clamp(34px, 6vw, 52px);
    font-weight: 800;
    font-style: italic;
    letter-spacing: -0.03em;
    margin: 8px 0 18px;
  }
  .filters {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 22px;
    min-width: 0;
  }
  .chips {
    display: flex;
    gap: 8px;
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    /* Тень у края — видно, что фильтры листаются. */
    mask-image: linear-gradient(90deg, #000 calc(100% - 24px), transparent);
    padding-right: 24px;
  }
  .find {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
  }
  .find.open {
    flex: 1 1 220px;
    max-width: 320px;
    padding-left: 12px;
    border-radius: 999px;
    border: 1.5px solid var(--line-strong);
  }
  .find input {
    flex: 1;
    min-width: 0;
    height: 34px;
    border: 0;
    outline: none;
    background: transparent;
  }
  .find input::-webkit-search-cancel-button {
    display: none;
  }
  .round {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    flex: none;
    border-radius: 50%;
    border: 0;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .round:hover {
    color: var(--text);
  }
  @media (max-width: 560px) {
    .filters:has(.find.open) .chips {
      display: none;
    }
    .find.open {
      max-width: none;
    }
  }

  .resume {
    margin-bottom: 26px;
  }
  h2 {
    font-size: 18px;
    font-weight: 700;
    margin-bottom: 12px;
  }
  .strip {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(250px, 300px);
    gap: 12px;
    overflow-x: auto;
    margin: 0 calc(var(--gutter) * -1);
    padding: 0 var(--gutter) 4px;
    scroll-snap-type: x proximity;
    scroll-padding: 0 var(--gutter);
  }
  .go {
    position: relative;
    display: flex;
    align-items: flex-end;
    min-width: 0;
    height: 120px;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--tint);
    scroll-snap-align: start;
  }
  .go img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0.55;
  }
  .go::before {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, var(--tint) 10%, transparent 90%);
  }
  .play {
    position: absolute;
    top: 12px;
    left: 12px;
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: #fff;
    color: #0a0a0a;
  }
  .txt {
    position: relative;
    display: grid;
    min-width: 0;
    padding: 0 14px 16px;
  }
  .txt b,
  .txt small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .txt small {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.75);
  }
  .go .bar {
    position: absolute;
    inset: auto 0 0;
    height: 4px;
    background: rgba(0, 0, 0, 0.45);
  }
  .go .bar i {
    display: block;
    height: 100%;
    background: #fff;
  }

  .ph {
    aspect-ratio: 20 / 27;
    border-radius: var(--radius);
  }
  .empty :global(.btn) {
    margin-top: 6px;
  }
</style>
