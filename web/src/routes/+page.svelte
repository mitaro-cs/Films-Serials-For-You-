<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api } from '$lib/api';
  import { catalog, loadCatalog } from '$lib/catalog.svelte';
  import PosterRow from '$lib/discover/PosterRow.svelte';
  import { extSheet } from '$lib/discover/sheet.svelte';
  import type { ExtCard, MarkItem } from '$lib/types';
  import { counts, featured, filter, ofTab, shelves, STATUSES, statusOf, TABS, tabOf, type Status } from '$lib/library';
  import { session } from '$lib/session.svelte';
  import Hero from '$lib/ui/Hero.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Shelf from '$lib/ui/Shelf.svelte';
  import Wide from '$lib/ui/Wide.svelte';

  // Главная как у Apple TV: герой-карусель и ряды широких карточек. Фильмы и Сериалы — герой,
  // «Продолжить», «Эксклюзив на Nova» (что хост добавил сам), «Буду смотреть» (с сервиса и из
  // Обзора), популярное и топ-250 IMDb, а внизу — весь эксклюзив сеткой с фильтром по статусу.
  const tab = $derived(tabOf(page.url.searchParams));
  const status = $derived(statusOf(page.url.searchParams));

  $effect(() => {
    loadCatalog();
  });

  const items = $derived(catalog.items ? ofTab(catalog.items, tab) : null);
  const hero = $derived(items ? featured(items) : []);
  const rows = $derived(items ? shelves(items, catalog.recent, tab) : []);
  const n = $derived(items ? counts(items) : null);
  const shown = $derived(items && tab !== 'home' ? filter(items, status) : []);

  // Ряды IMDb и «Буду смотреть» — только во вкладках; пока грузятся, низ страницы не рисуем
  // (иначе сетка ниже сдвигалась бы).
  type Feed = { rows: { key: string; title: string; items: ExtCard[] }[] };
  let feed = $state<Feed | null>(null);
  let marks = $state<{ want: MarkItem[]; watched: MarkItem[] } | null>(null);
  $effect(() => {
    if (tab === 'home') return;
    void extSheet.version;
    Promise.all([
      api<Feed>('/api/discover').catch(() => ({ rows: [] }) as Feed),
      api<{ want: MarkItem[]; watched: MarkItem[] }>('/api/me/marks').catch(() => ({ want: [], watched: [] })),
    ]).then(([f, m]) => {
      feed = f;
      marks = m;
    });
  });
  const kind = $derived(tab === 'series' ? 'series' : 'movie');
  const imdb = $derived(feed ? feed.rows.filter((r) => r.key.endsWith(tab === 'series' ? '_series' : '_movies')) : []);
  const want = $derived(marks ? marks.want.filter((m) => (m.card ?? m.ext)?.kind === kind) : []);
  const exclusive = $derived(items ? [...items].sort((a, b) => b.addedAt - a.addedAt).slice(0, 24) : []);
  const ready = $derived(feed !== null && marks !== null);

  // Сетка всей коллекции рисуется частями: сотни карточек разом подтормаживали прокрутку
  // и открытие вкладки. Следующие 48 — когда до конца остаётся экран.
  const STEP = 48;
  let limit = $state(STEP);
  let more = $state<HTMLElement>();
  $effect(() => {
    void tab;
    void status;
    limit = STEP;
  });
  $effect(() => {
    if (!more) return;
    const io = new IntersectionObserver((e) => e[0].isIntersecting && (limit += STEP), { rootMargin: '100% 0px' });
    io.observe(more);
    return () => io.disconnect();
  });

  function setStatus(s: Status) {
    const u = new URL(page.url);
    if (s === 'all') u.searchParams.delete('status');
    else u.searchParams.set('status', s);
    goto(u, { replaceState: true, noScroll: true, keepFocus: true });
  }

  const EMPTY: Record<Status, string> = {
    all: 'Здесь пока пусто',
    want: 'Отметьте «Хочу посмотреть» — кнопкой «+» на странице фильма',
    watching: 'Начатого нет — всё досмотрено',
    watched: 'Досмотренное появится здесь',
  };
</script>

<div class="home" class:flat={items !== null && !hero.length}>
  <!-- Ниже 1000 px капсулы в шапке нет — разделы каталога здесь, поверх кадра. -->
  <nav class="kinds" aria-label="Разделы каталога">
    {#each TABS as t (t.id)}
      <a href={t.href} class:on={tab === t.id} aria-current={tab === t.id ? 'page' : undefined}>{t.label}</a>
    {/each}
  </nav>

  {#if catalog.failed}
    <div class="page empty"><Icon name="offline" size={40} /> Не удалось загрузить — проверьте связь</div>
  {:else if items === null}
    <div class="ph-hero skeleton" aria-busy="true"></div>
    <div class="rows">
      {#each Array(2) as _, r (r)}
        <div class="ph-row">
          <span class="ph-title skeleton"></span>
          <div class="ph-cards">{#each Array(6) as _, i (i)}<span class="ph-card skeleton"></span>{/each}</div>
        </div>
      {/each}
    </div>
  {:else if items.length === 0 && tab === 'home'}
    <div class="page empty">
      <Icon name="film" size={40} />
      {#if session.me?.manage}
        <p>Здесь пока пусто.</p>
        <a class="btn primary sm" href="/admin?tab=folders">Добавить фильмы</a>
      {:else}
        <p>Здесь пока пусто — хост ещё не добавил фильмы.</p>
      {/if}
    </div>
  {:else}
    {#if hero.length}<Hero items={hero} />{/if}

    <div class="rows">
      {#if tab === 'home'}
        {#each rows as row, i (row.key)}<Shelf title={row.title} items={row.items} eager={i < 2} {i} />{/each}
      {:else}
        {#each rows.filter((r) => r.key === 'continue') as row (row.key)}<Shelf title={row.title} items={row.items} eager />{/each}
        {#if !ready}
          {#each Array(2) as _, r (r)}
            <div class="ph-row">
              <span class="ph-title skeleton"></span>
              <div class="ph-cards">{#each Array(6) as _, i (i)}<span class="ph-card skeleton"></span>{/each}</div>
            </div>
          {/each}
        {:else}
        {#if exclusive.length}<Shelf title="Эксклюзив на Nova" items={exclusive} i={1} />{/if}
        {#if want.length}<PosterRow title="Буду смотреть" items={want} i={2} />{/if}
        {#each imdb as row, j (row.key)}
          <PosterRow title={row.title} items={row.items} href="/discover?chart={row.key}" ranked={row.key.startsWith('top')} i={j + 3} />
        {/each}
        <section aria-label="Вся коллекция" class="rise" style:--i={5}>
          <div class="head">
            <h2>Эксклюзив — {tab === 'movies' ? 'все фильмы' : 'все сериалы'}</h2>
            <div class="chips" role="tablist" aria-label="Статус">
              {#each STATUSES as s (s.id)}
                <button class="chip" class:on={status === s.id} role="tab" aria-selected={status === s.id} onclick={() => setStatus(s.id)}>
                  {s.label}
                  {#if n && n[s.id]}<small>{n[s.id]}</small>{/if}
                </button>
              {/each}
            </div>
          </div>
          {#if shown.length}
            <div class="grid">
              {#each shown.slice(0, limit) as card, i (card.id)}<Wide {card} eager={i < 8} i={i % STEP} />{/each}
            </div>
            {#if shown.length > limit}<div bind:this={more} class="more" aria-hidden="true"></div>{/if}
          {:else if !items.length}
            <div class="empty">
              {#if session.me?.manage}
                <p>{tab === 'series' ? 'Своих сериалов пока нет' : 'Своих фильмов пока нет'} — добавьте их с компьютера.</p>
                <a class="btn primary sm" href="/admin?tab=folders">Добавить</a>
              {:else}
                <p>{tab === 'series' ? 'Сериалов' : 'Фильмов'} от хоста пока нет — посмотрите популярное выше.</p>
              {/if}
            </div>
          {:else}
            <div class="empty">{EMPTY[status]}</div>
          {/if}
        </section>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  /* --row — высота первого ряда: он лежит на нижней части кадра героя, как у Apple TV. */
  .home {
    --row: clamp(240px, 31vh, 330px);
    min-height: 100dvh;
    padding-bottom: calc(48px + var(--tabbar));
  }
  .kinds {
    display: none;
  }
  @media (max-width: 1000px) {
    .kinds {
      position: absolute;
      z-index: 5;
      top: calc(var(--header) + env(safe-area-inset-top) + 4px);
      left: 50%;
      translate: -50% 0;
      display: flex;
      gap: 2px;
      padding: 4px;
      border-radius: 999px;
      background: color-mix(in srgb, var(--hero-tint, #2a2724) 45%, rgba(20, 20, 20, 0.84));
      transition: background 0.5s;
      box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
    }
    .kinds a {
      padding: 6px 16px;
      border-radius: 999px;
      color: rgba(255, 255, 255, 0.82);
      font-size: 14px;
      font-weight: 600;
      white-space: nowrap;
      transition:
        background 0.2s,
        color 0.2s;
    }
    .kinds a.on {
      background: rgba(235, 235, 240, 0.92);
      color: #111;
    }
    /* Без героя капсула — над содержимым. */
    .flat .kinds {
      position: relative;
      top: auto;
      left: auto;
      translate: none;
      width: max-content;
      margin: calc(var(--header) + env(safe-area-inset-top) + 4px) auto 0;
    }
  }
  /* Ряды «лежат» на нижней части кадра, как у Apple TV. */
  .rows {
    position: relative;
    z-index: 1;
    margin-top: calc(-1 * var(--row));
    padding: 0 var(--gutter);
  }
  .flat .rows {
    margin-top: 0;
    padding-top: calc(var(--header) + 24px);
  }
  @media (max-width: 1000px) {
    .flat .rows {
      padding-top: 20px;
    }
  }
  @media (max-width: 640px) {
    .home {
      --row: 200px;
    }
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px 20px;
    margin-bottom: 16px;
  }
  h2 {
    font-size: clamp(19px, 1.6vw, 24px);
    font-weight: 700;
  }
  .chips {
    display: flex;
    gap: 8px;
    min-width: 0;
    overflow-x: auto;
  }
  .ph-hero {
    height: clamp(560px, 100svh, 1200px);
    border-radius: 0;
  }
  @media (max-width: 640px) {
    .ph-hero {
      height: clamp(520px, 86svh, 760px);
    }
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
    grid-auto-columns: calc((100% - 4.3 * 18px) / 5.3);
    gap: 18px;
    overflow: hidden;
  }
  .ph-card {
    aspect-ratio: 16 / 9;
    border-radius: 12px;
  }
  @media (max-width: 640px) {
    .ph-cards {
      grid-auto-columns: calc((100% - 12px) / 1.75);
      gap: 12px;
    }
  }
  .more {
    height: 1px;
  }
  .empty :global(.btn) {
    margin-top: 6px;
  }
</style>
