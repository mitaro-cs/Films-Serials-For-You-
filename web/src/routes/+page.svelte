<script lang="ts">
  import { progressSync } from '$lib/progressSync.svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api } from '$lib/api';
  import { catalog, loadCatalog } from '$lib/catalog.svelte';
  import PosterRow from '$lib/discover/PosterRow.svelte';
  import { extSheet } from '$lib/discover/sheet.svelte';
  import type { ExtCard, Marks, ResumeItem } from '$lib/types';
  import ContinueRow from '$lib/ui/ContinueRow.svelte';
  import Loading from '$lib/ui/Loading.svelte';
  import BrowseGenres from '$lib/ui/BrowseGenres.svelte';
  import { counts, featured, filter, isAnime, ofTab, shelves, STATUSES, statusOf, tabOf, type Status } from '$lib/library';
  import { session } from '$lib/session.svelte';
  import Hero from '$lib/ui/Hero.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Shelf from '$lib/ui/Shelf.svelte';
  import Wide from '$lib/ui/Wide.svelte';
  import Showcase from '$lib/ui/Showcase.svelte';

  // Главная как у Apple TV: герой-карусель и ряды широких карточек. Фильмы и Сериалы - герой,
  // «Продолжить», «Эксклюзив на Nova» (что хост добавил сам), «Буду смотреть» (с сервиса и из
  // Обзора), популярное и топ-250 IMDb, а внизу - весь эксклюзив сеткой с фильтром по статусу.
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

  // Ряды IMDb и «Буду смотреть» - только во вкладках; пока грузятся, низ страницы не рисуем
  // (иначе сетка ниже сдвигалась бы).
  type Feed = { rows: { key: string; title: string; items: ExtCard[] }[] };
  let feed = $state<Feed | null>(null);
  let marks = $state<Marks | null>(null);
  let resumes = $state<ResumeItem[]>([]);
  let resumeFailed = $state(false);
  // Смотрели на другом устройстве - «Продолжить» и каталог (места в карточках) перечитываются.
  $effect(() => {
    if (progressSync.stamp) loadCatalog(true);
  });
  $effect(() => {
    void extSheet.version;
    void progressSync.stamp;
    let live = true;
    api<ResumeItem[]>('/api/continue').then((r) => { if (live) { resumes = r; resumeFailed = false; } }, () => { if (live) resumeFailed = true; });
    return () => { live = false; };
  });
  const sectionResumes = $derived(resumes.filter((r) => tab === 'home' ? true : tab === 'anime' ? isAnime(r.card ?? r.ext!) : (r.card ?? r.ext)?.kind === (tab === 'series' ? 'series' : 'movie')));
  // Тема недели: ряд под «Продолжить» и пара кадров в герое.
  let theme = $state<{ key: string; title: string; subtitle: string; items: ExtCard[] } | null>(null);
  $effect(() => {
    void extSheet.version;
    let live = true;
    api<NonNullable<typeof theme>>('/api/theme').then((t) => { if (live) theme = t; }, () => {});
    return () => { live = false; };
  });
  let feedFailed = $state(false);
  let generation = 0;
  let refresh = $state(0);
  $effect(() => {
    void extSheet.version;
    void refresh;
    const request = ++generation;
    Promise.all([
      api<Feed>('/api/discover').then((f) => { feedFailed = false; return f; }, () => { feedFailed = true; return { rows: [] } as Feed; }),
      api<Marks>('/api/me/marks').catch(() => ({ want: [], watching: [], watched: [] })),
    ]).then(([f, m]) => {
      if (request !== generation) return;
      feed = f;
      marks = m;
    });
  });
  const kind = $derived(tab === 'series' ? 'series' : 'movie');
  const imdb = $derived(feed ? feed.rows.filter((r) => tab === 'home' ? !r.key.startsWith('anime_') : tab === 'anime' ? r.key.startsWith('anime_') : r.key.endsWith(tab === 'series' ? '_series' : '_movies')) : []);
  // Герой: сначала своё (с местом, где остановились), потом ваши «Смотрю» и «Хочу» из Обзора, потом
  // тема недели и популярное IMDb - без повторов и брошенного.
  const showcase = $derived.by(() => {
    const local = hero.slice(0, 3).map((card) => ({ card }));
    const ok = (e: ExtCard) => e.mark !== 'dropped' && !e.local;
    const saved = marks ? [...marks.watching, ...marks.want].flatMap((m) => (m.ext && ok(m.ext) ? [m.ext] : [])) : [];
    const themed = (theme?.items ?? []).filter(ok).slice(0, 2);
    const popular = imdb.flatMap((r) => r.items).filter(ok);
    const ext = [...new Map([...saved.slice(0, 3), ...themed, ...popular].map((e) => [e.id, e])).values()];
    return [...local, ...ext.map((e) => ({ ext: e }))].slice(0, 6);
  });
  const animeIds = $derived(new Set(feed?.rows.filter((r) => r.key.startsWith('anime_')).flatMap((r) => r.items.map((c) => c.id)) ?? []));
  const inSection = (m: Pick<Marks['want'][number], 'card' | 'ext'>) => tab === 'anime'
    ? !!((m.card && isAnime(m.card)) || (m.ext && (isAnime(m.ext) || animeIds.has(m.ext.id))))
    : tab === 'home' || (m.card ?? m.ext)?.kind === kind;
  const want = $derived(marks ? marks.want.filter(inSection) : []);
  // «Смотрю» из Обзора и отмеченное вручную (начатое с сервиса - уже в «Продолжить»).
  const going = $derived(marks ? marks.watching.filter((m) => inSection(m) && !resumes.some((r) => r.key === (m.card ? `t:${m.card.id}` : `x:${m.ext?.id}`))) : []);
  const favorites = $derived((marks?.favorites ?? []).filter(inSection));
  const ready = $derived(feed !== null && marks !== null);

  // Сетка всей коллекции рисуется частями: сотни карточек разом подтормаживали прокрутку
  // и открытие вкладки. Следующие 48 - когда до конца остаётся экран.
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
    const u = new URL(page.url.href);
    if (s === 'all') u.searchParams.delete('status');
    else u.searchParams.set('status', s);
    goto(u, { replace: true, reset: false });
  }

  const EMPTY: Record<Status, string> = {
    all: 'Здесь пока пусто',
    want: 'Отметьте «Хочу посмотреть» - кнопкой «+» на странице фильма',
    watching: 'Начатого нет - всё досмотрено',
    watched: 'Досмотренное появится здесь',
  };
</script>

<div class="home" class:flat={tab !== 'home' && items !== null && !hero.length} class:homepage={tab === 'home'}>
  {#if catalog.failed}
    <div class="page empty"><Icon name="offline" size={40} /> Не удалось загрузить - проверьте связь</div>
  {:else if items === null}
    <div class="ph-hero"><Loading label="Собираем ваш кинотеатр" /></div>
    <div class="rows">
      {#each Array(2) as _, r (r)}
        <div class="ph-row">
          <span class="ph-title skeleton"></span>
          <div class="ph-cards">{#each Array(6) as _, i (i)}<span class="ph-card skeleton"></span>{/each}</div>
        </div>
      {/each}
    </div>
  {:else}
    {#if tab === 'home'}
      {#if showcase.length}<Showcase items={showcase} />{:else}
        <section class="welcome">
          <span class="eyebrow">ВАШ ДОМАШНИЙ КИНОТЕАТР</span>
          <h1>Что посмотрим сегодня?</h1>
          <p>Фильмы, сериалы и истории, к которым хочется возвращаться.</p>
          <div class="welcome-actions"><a class="btn primary" href="/discover">Открыть обзор</a><a class="btn" href="/search">Найти фильм</a>{#if session.me?.manage}<a class="btn" href="/admin?tab=folders">Добавить с компьютера</a>{/if}</div>
        </section>
      {/if}
    {:else if hero.length}<Hero items={hero} />{/if}

    <div class="rows">
      {#if tab === 'home'}
        {#if sectionResumes.length}<ContinueRow items={sectionResumes} />{:else if resumeFailed}{#each rows.filter((r) => r.key === 'continue') as row (row.key)}<Shelf title={row.title} items={row.items} eager droppable />{/each}{/if}
        {#if theme?.items.length}<PosterRow title={theme.title} note={theme.subtitle || 'тема недели'} items={theme.items} />{/if}
        {#if going.length}<PosterRow title="Смотрю сейчас" items={going} />{/if}
        {#if favorites.length}<PosterRow title="Избранное" items={favorites} />{/if}
        {#if want.length}<PosterRow title="В планах на вечер" items={want} />{/if}
        {#each rows.filter((r) => r.key !== 'continue' && r.key !== 'want').slice(0, 3) as row, i (row.key)}<PosterRow title={row.title} items={row.items.map((card) => ({ card }))} {i} />{/each}
        {#if !ready}
          <section class="feed-loading" aria-busy="true"><h2>Подбираем истории для вас</h2><div class="ph-cards">{#each Array(6) as _, i (i)}<span class="ph-card skeleton"></span>{/each}</div></section>
        {:else}
          {#each imdb.slice(0, 6) as row, i (row.key)}<PosterRow title={row.title} items={row.items} href="/discover?chart={row.key}" ranked={row.key.startsWith('top')} {i} />{/each}
          <BrowseGenres />
          {#if !imdb.some((r) => r.items.length)}
            <section class="feed-empty"><h2>{feedFailed ? 'Подборки временно недоступны' : 'Выберите свою следующую историю'}</h2><p class="muted">{feedFailed ? 'Не удалось получить подборки. Ваша библиотека и поиск остаются под рукой.' : 'Находите фильмы через поиск и добавляйте их в планы на вечер.'}</p><div class="welcome-actions"><button class="btn" onclick={() => refresh++}>Обновить подборки</button><a class="btn" href="/search">Перейти к поиску</a></div></section>
          {/if}
        {/if}
      {:else}
        {#if sectionResumes.length}<ContinueRow items={sectionResumes} />{:else if resumeFailed}{#each rows.filter((r) => r.key === 'continue') as row (row.key)}<Shelf title={row.title} items={row.items} eager droppable />{/each}{/if}
        {#if !ready}
          {#each Array(2) as _, r (r)}
            <div class="ph-row">
              <span class="ph-title skeleton"></span>
              <div class="ph-cards">{#each Array(6) as _, i (i)}<span class="ph-card skeleton"></span>{/each}</div>
            </div>
          {/each}
        {:else}
        {#if going.length}<PosterRow title="Смотрю" items={going} i={2} />{/if}
        {#if want.length}<PosterRow title="Буду смотреть" items={want} i={2} />{/if}
        {#if favorites.length}<PosterRow title="Избранное" items={favorites} />{/if}
        {#each imdb as row, j (row.key)}
          <PosterRow title={row.title} items={row.items} href="/discover?chart={row.key}" ranked={row.key.startsWith('top')} i={j + 3} />
        {/each}
        <section aria-label="Вся коллекция" class="rise" style:--i={5}>
          <div class="head">
            <h2>На Nova - {tab === 'anime' ? 'аниме' : tab === 'movies' ? 'все фильмы' : 'все сериалы'}</h2>
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
                <p>{tab === 'anime' ? 'Своего аниме пока нет' : tab === 'series' ? 'Своих сериалов пока нет' : 'Своих фильмов пока нет'} - добавьте с компьютера.</p>
                <a class="btn primary sm" href="/admin?tab=folders">Добавить</a>
              {:else}
                <p>{tab === 'anime' ? 'Аниме' : tab === 'series' ? 'Сериалов' : 'Фильмов'} на Nova пока нет - посмотрите подборки выше.</p>
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
  .ph-hero { display: grid; place-items: center; background: #000; }
  .homepage .rows { margin-top: 0; padding-top: 12px; }
  .welcome { min-height: 430px; padding: calc(var(--header) + 80px) var(--gutter) 48px; background: radial-gradient(ellipse at 80% 20%, #37302566, transparent 65%); }
  .welcome h1 { font-size: clamp(32px, 4vw, 64px); letter-spacing: -.04em; margin: 18px 0; }
  .welcome p { color: var(--muted); font-size: 17px; }
  .eyebrow { color: #f5c518; letter-spacing: .12em; font-size: 11px; font-weight: 800; }
  .welcome-actions { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 24px; }
  .feed-empty { padding: 28px; border: 1px solid var(--line); border-radius: 18px; margin: 20px 0; }
  .feed-loading { margin-bottom: 30px; }
  .feed-loading h2 { margin-bottom: 16px; }
  /* --row - высота первого ряда: он лежит на нижней части кадра героя, как у Apple TV. */
  .home {
    --row: clamp(240px, 31vh, 330px);
    min-height: 100dvh;
    padding-bottom: calc(48px + var(--tabbar));
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
      padding-top: calc(var(--header) + env(safe-area-inset-top) + 20px);
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
