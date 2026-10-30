<script lang="ts" module>
  /** Серия в ленте: файл с сервиса (ссылка на плеер) или серия из ленты IMDb (отметка «посмотрел»). */
  export interface Ep {
    key: string;
    season: number;
    episode: number | null;
    name: string;
    overview: string | null;
    still: string | null;
    minutes: number | null;
    rating: number | null;
    date: string | null;
    /** 0..100 - сколько посмотрено. */
    pct: number;
    done: boolean;
    /** «осталось 23 мин». */
    left: string | null;
    href?: string;
    current: boolean;
    upcomingYear?: number | null;
  }
</script>

<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import type { Snippet } from 'svelte';
  import { date as fmtDate } from '$lib/fmt';
  import { artSet, artW, placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import { localized } from '$lib/localized.svelte';
  import { fitTitle } from '$lib/ui/fitTitle';

  // Сериал как у HBO GO: слева большой кадр, жанр и год, крупное название с оценкой, полоска
  // «где остановился», кнопки; справа - лента серий всех сезонов подряд (текущая - светлая и
  // крупнее, лента сама прокручена к ней), у края - сезоны. На телефоне - одной колонкой.
  let {
    title,
    art,
    meta,
    rating = null,
    age = null,
    tagline = null,
    progress = null,
    episodes,
    seasons,
    loading = false,
    onseason,
    onpick,
    onseen,
    onsource,
    slim = false,
    actions,
    extra,
  }: {
    title: string;
    art: string | null;
    meta: string[];
    rating?: number | null;
    age?: string | null;
    tagline?: string | null;
    progress?: { label: string; pct: number } | null;
    episodes: Ep[];
    seasons: number[];
    loading?: boolean;
    /** Сезона ещё нет в ленте (сериал из ленты грузит по сезону) - попросить. */
    onseason?: (s: number) => void;
    /** Нажали серию без ссылки (сериал из ленты) - отметить. */
    onpick?: (e: Ep) => void;
    /** Галочка «посмотрел» у серии (сериал из ленты). */
    onseen?: (e: Ep) => void;
    onsource?: (e: Ep) => void;
    /** Телефон: кадр, название, описание и «Смотреть» - в MobileHero над этим блоком. */
    slim?: boolean;
    actions: Snippet;
    extra?: Snippet;
  } = $props();

  let list = $state<HTMLElement>();
  let shown = $state<number | null>(null);

  const groups = $derived.by(() => {
    const out: { season: number; items: Ep[] }[] = [];
    for (const e of episodes) {
      const g = out.at(-1);
      if (g && g.season === e.season) g.items.push(e);
      else out.push({ season: e.season, items: [e] });
    }
    return out;
  });
  // Сезоны по порядку: первый - сверху.
  const rail = $derived(seasons.length ? [...seasons].sort((a, b) => a - b) : []);

  // Сезон переключается без прокрутки страницы.
  function go(s: number) {
    shown = s;
    if (!groups.some(g => g.season === s)) onseason?.(s);
  }

  const activeEpisode = $derived(episodes.find((e) => e.current && !e.upcomingYear) ?? episodes.find((e) => !e.done && !e.upcomingYear) ?? episodes.find(e => !e.upcomingYear));
  const cur = $derived(shown ?? episodes.find((e) => e.current)?.season ?? seasons[0] ?? null);
</script>

{#snippet card(e: Ep)}
  <span class="still" style:background={e.upcomingYear ? 'linear-gradient(135deg,#26282c,#111216)' : e.still ? undefined : placeholder(title + e.key)}>
    {#if e.upcomingYear}<span class="still-label"><Icon name="film" size={22} /> Скоро</span>{:else if e.still}<img use:fadeIn src={artW(e.still, 640)} alt="" loading="lazy" decoding="async" width="320" height="180" />{:else}<span class="still-label"><Icon name="film" size={22} />S{e.season} · E{e.episode ?? 1}</span>{/if}
    {#if !e.upcomingYear && (e.href || onpick)}<span class="play"><Icon name="play" size={16} /></span>{/if}
    {#if e.done}<span class="tick" title="Просмотрено"><Icon name="check" size={13} stroke={3} /></span>{/if}
  </span>
  <span class="body">
    <span class="row1">
      <b class="name">{e.upcomingYear ? `Выйдет в ${e.upcomingYear} году` : `${e.episode !== null ? `${e.episode}. ` : ''}${localized(e.name, 'Серия')}`}</b>
      <small class="len">{e.left ?? (e.minutes ? `${e.minutes} мин` : '')}</small>
    </span>
    {#if e.done}<span class="watch-state"><Icon name="check" size={12} stroke={3}/> Просмотрено</span>{/if}
    {#if e.pct > 0 && !e.done}<span class="pbar"><i style:width="{e.pct}%"></i></span>{/if}
    <span class="row2">
      {#if localized(e.overview)}<span class="ov">{localized(e.overview)}</span>{:else if e.rating}<span class="star"><Icon name="star" size={12} /> {e.rating.toFixed(1)}</span>{:else}<span></span>{/if}
      <small class="when">{fmtDate(e.date)}</small>
    </span>
    {#if localized(e.overview) && e.rating}<span class="star small"><Icon name="star" size={11} /> {e.rating.toFixed(1)}</span>{/if}
  </span>
{/snippet}

<section class="sh" class:slim aria-label="Сериал «{title}»">
  <div class="bg" aria-hidden="true">
    {#key art}
      {#if art}<img use:fadeIn src={art} srcset={artSet(art)} sizes="100vw" alt="" decoding="async" />{:else}<div class="ph" style:background={placeholder(title)}></div>{/if}
    {/key}
  </div>

  <div class="info">
    <p class="meta">
      {#each meta as m, k (k)}<span>{m}</span>{/each}
      {#if age}<span class="age">{age}</span>{/if}
    </p>
    <h1 use:fitTitle={title} title={title}>{title}</h1>
    {#if rating}<p class="rating"><Icon name="star" size={16} /> {rating.toFixed(1)} <small>IMDb</small></p>{/if}
    {#if activeEpisode}
      <div class="viewing-progress" aria-label="Прогресс просмотра">
        <div class="progress-head"><b>{activeEpisode ? `${activeEpisode.season} сезон · ${activeEpisode.episode ?? 1} серия` : 'Ваш прогресс'}</b><span>{activeEpisode?.left ?? (activeEpisode?.pct ? `${Math.round(activeEpisode.pct)}%` : 'С начала')}</span></div>
        <div class="progress-seasons">
          {#each groups as g (g.season)}
            <button class="season-progress" onclick={() => go(g.season)} aria-label="{g.season} сезон: просмотрено {g.items.filter((e) => e.done).length} из {g.items.length}">
              <span>{g.season} сезон</span><span class="segments">{#each g.items as ep (ep.key)}<i class:done={ep.done} class:current={ep.current} title="Серия {ep.episode ?? 1}: {ep.done ? 'просмотрено' : ep.pct > 0 ? `${Math.round(ep.pct)}%` : 'не просмотрено'}" style:--pct="{ep.pct}%"></i>{/each}</span><small>{g.items.filter((e) => e.done).length}/{g.items.length}</small>
            </button>
          {/each}
        </div>
        <small class="progress-note">{progress?.label ?? (activeEpisode?.pct ? 'Продолжим с сохранённого момента' : 'Место просмотра сохранится автоматически')}</small>
      </div>
    {/if}
    {#if localized(tagline)}<p class="tagline">{localized(tagline)}</p>{/if}
    <div class="acts">{@render actions()}</div>
    {@render extra?.()}
  </div>

  <div class="eps" bind:this={list} aria-label="Серии" role="region">
    {#if rail.length > 1}
      <div class="chips" role="tablist" aria-label="Сезоны">
        {#each rail as s (s)}
          <button class="chip" class:on={cur === s} role="tab" aria-selected={cur === s} aria-label="{s} сезон" onclick={() => go(s)}><span class="full">{s} сезон</span><span class="short">Сезон {s}</span></button>
        {/each}
      </div>
    {/if}
    {#each groups.filter(g => g.season === cur) as g (g.season)}
      {#if seasons.length > 1 || groups.length > 1}<h2 class="sea" class:other={g.season !== cur} data-season={g.season}>{g.season} сезон</h2>{:else}<span class="sea0" data-season={g.season}></span>{/if}
      <ol class:other={g.season !== cur}>
        {#each g.items as e, i (e.key)}
          <li class="ep" class:done={e.done} class:cur={e.current} style:--i={i}>
            {#if e.upcomingYear}
              <div class="card" aria-label="Выйдет в {e.upcomingYear} году">{@render card(e)}</div>
            {:else if e.href}
              <a class="card" href={e.href} aria-current={e.current ? 'true' : undefined}>{@render card(e)}</a>
              {#if onsource && e.episode !== null}<button class="source" aria-label="Другие источники: {e.season} сезон, {e.episode} серия" title="Другие источники" onclick={() => onsource?.(e)}><Icon name="link" size={14} /></button>{/if}
            {:else}
              <!-- Нажали - эта серия выбрана; галочка рядом - «посмотрел» (отдельная кнопка). -->
              <button class="card" aria-current={e.current ? 'true' : undefined} aria-label="{e.episode ?? ''} серия: {localized(e.name, 'Серия')}" onclick={() => onpick?.(e)}>{@render card(e)}</button>
              {#if onseen}
                <button class="seen" class:on={e.done} aria-pressed={e.done} aria-label="Посмотрел {e.episode ?? ''} серию" title={e.done ? 'Снять «посмотрел»' : 'Посмотрел (и все раньше)'} onclick={() => onseen?.(e)}>
                  <Icon name="check" size={14} stroke={3} />
                </button>
              {/if}
            {/if}
          </li>
        {/each}
      </ol>
    {/each}
    {#if loading}
      {#each Array(4) as _, k (k)}<div class="ep-sk skeleton"></div>{/each}
    {:else if !groups.some(g => g.season === cur)}
      <p class="none muted">Серий в этом сезоне пока нет.</p>
    {/if}
  </div>

  {#if rail.length > 1}
    <!-- Сезоны у края: дорожка светится до выбранного, капсула перетекает к нему. -->
    <nav class="rail" aria-label="Сезоны сбоку" style:--n={rail.length} style:--at={Math.max(0, rail.indexOf(cur ?? rail[0]))}>
      <small class="cap">Сезон</small>
      <div class="col">
        <span class="pill" aria-hidden="true"></span>
        {#each rail as s (s)}
          <button class:on={cur === s} onclick={() => go(s)} aria-label="{s} сезон" aria-current={cur === s ? "true" : undefined}>{s}</button>
        {/each}
      </div>
    </nav>
  {/if}
</section>

<style>
  .source { align-self: center; margin: 8px; width: 28px; height: 28px; display: grid; place-items: center; border: 1px solid #ffffff55; border-radius: 50%; background: #111e; color: white; cursor: pointer; }
  .sh {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.15fr) 56px;
    gap: 0 28px;
    height: clamp(640px, 100svh, 980px);
    padding: calc(var(--header) + 20px) var(--gutter) 28px;
    overflow: hidden;
    isolation: isolate;
  }
  /* Кадр - на всю ширину: слева ярко, под лентой серий приглушён (не чёрная пустота). Свой слой
     обрезает «дыхание» кадра - иначе его край выглядывал полосой. */
  .bg {
    position: absolute;
    inset: 0;
    z-index: -1;
    overflow: hidden;
  }
  .bg img,
  .ph {
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: 30% 20%;
    animation: drift 30s ease-in-out infinite alternate;
  }
  @keyframes drift {
    to {
      transform: scale(1.06) translateX(-1.5%);
    }
  }
  .bg::after {
    content: '';
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, transparent 22%, rgba(0, 0, 0, 0.72) 48%, rgba(0, 0, 0, 0.8) 100%),
      linear-gradient(0deg, var(--bg) 0%, rgba(0, 0, 0, 0.55) 22%, transparent 55%),
      linear-gradient(180deg, rgba(0, 0, 0, 0.55), transparent 22%);
  }

  .info {
    align-self: end;
    display: grid;
    gap: 12px;
    justify-items: start;
    min-width: 0;
    padding-bottom: 12px;
    animation: rise-in 0.6s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    margin: 0;
    font-size: 13.5px;
    color: rgba(255, 255, 255, 0.78);
  }
  .age {
    padding: 0 6px;
    border: 1px solid rgba(255, 255, 255, 0.4);
    border-radius: 4px;
    font-size: 12px;
  }
  h1 {
    max-width: 100%;
    font-size: clamp(34px, 4.6vw, 64px);
    font-weight: 800;
    letter-spacing: -0.02em;
    line-height: 1.02;
    text-transform: uppercase;
    white-space: nowrap;
    text-overflow: ellipsis;
    text-shadow: 0 4px 30px rgba(0, 0, 0, 0.5);
    display: block;
    overflow: hidden;
  }
  .rating {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: -4px 0 0;
    font-weight: 700;
    color: var(--imdb);
  }
  .rating small {
    color: var(--muted);
    font-weight: 600;
  }
  .pbar {
    position: relative;
    display: block;
    height: 3px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.22);
    overflow: hidden;
  }
  .viewing-progress { max-width: 520px; padding: 14px; border: 1px solid #ffffff24; border-radius: 12px; background: #0c0d10b3; }
  .progress-head { display: flex; justify-content: space-between; gap: 12px; font-size: 12px; margin-bottom: 10px; }
  .progress-head span { color: #bfc0c8; font-variant-numeric: tabular-nums; }
  .progress-seasons { max-height: 110px; overflow-y: auto; display: grid; gap: 8px; }
  .season-progress { display: grid; grid-template-columns: 54px minmax(0,1fr) 34px; align-items: center; gap: 10px; padding: 3px 0; border: 0; color: #d1d1d6; background: transparent; font: inherit; font-size: 11px; cursor: pointer; text-align: left; }
  .season-progress small { text-align: right; }
  .segments { display: flex; gap: 3px; height: 9px; }
  .segments i { flex: 1; min-width: 1px; background: linear-gradient(90deg,#9cabc6 var(--pct),#ffffff20 var(--pct)); border-radius: 2px; }
  .segments i.done { background: #f5f5f7; }
  .segments i.current { outline: 1px solid #f5c518; outline-offset: 1px; }
  .progress-note { display: block; color: #aaaab3; font-size: 10px; margin-top: 9px; line-height: 1.5; }
  .pbar i {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: #000; outline: 1px solid #ffffff70;
    transform-origin: left;
    animation: grow 0.9s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  @keyframes grow {
    from {
      transform: scaleX(0);
    }
  }
  .tagline {
    margin: 0;
    max-width: 52ch;
    color: var(--muted);
    font-style: normal;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
    max-width: 100%;
  }

  /* Лента серий: своя прокрутка, края мягко гаснут. */
  .eps {
    min-width: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 18vh 28px 26vh;
    margin: 0 -28px;
    mask-image: linear-gradient(180deg, transparent 0, #000 14%, #000 82%, transparent 100%);
    scroll-behavior: smooth;
  }
  .eps .chips {
    display: none;
  }
  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 14px;
  }
  .sea {
    margin: 26px 0 14px;
    font-size: 14px;
    font-weight: 800;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.9);
  }
  .sea0 {
    display: block;
    height: 0;
  }
  .ep {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    content-visibility: auto;
    contain-intrinsic-size: auto 100px;
    animation: rise-in 0.5s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(min(var(--i, 0), 10) * 35ms);
  }
  /* Галочка «посмотрел» у серии из ленты - в углу кадра, своей кнопкой. */
  .seen {
    align-self: center;
    margin: 8px;
    z-index: 1;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.65);
    box-shadow: inset 0 0 0 1.5px rgba(255, 255, 255, 0.5);
    color: rgba(255, 255, 255, 0.55);
    cursor: pointer;
    transition:
      background 0.2s,
      color 0.2s,
      transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .seen:hover {
    color: #fff;
    transform: scale(1.12);
  }
  .seen.on {
    background: #000; outline: 1px solid #ffffff70;
    color: #fff;
    box-shadow: none;
    animation: pop 0.4s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .cur .seen {
    color: #fff;
  }
  .card {
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr);
    width: 100%;
    min-height: 88px;
    padding: 0;
    border: 0;
    border-radius: 10px;
    overflow: hidden;
    background: #000;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    box-shadow: 0 10px 30px -18px rgba(0, 0, 0, 0.9);
    transition:
      transform 0.35s cubic-bezier(0.2, 0.9, 0.3, 1.2),
      background 0.25s,
      color 0.25s;
  }
  @media (hover: hover) {
    .card:hover {
      transform: none;
      background: #080808;
    }
  }
  .card:active {
    transform: scale(0.98);
  }
  /* Текущая серия - светлая карточка крупнее остальных, выступает влево. */
  .cur .card {
    background: #000; outline: 1px solid #ffffff70;
    color: #fff;
    transform: none;
    box-shadow: 0 24px 50px -16px rgba(0, 0, 0, 0.9);
  }
  .cur .card:hover {
    background: #000; outline: 1px solid #ffffff70;
    transform: none;
  }
  .still {
    position: relative;
    display: block;
    width: 100%;
    min-width: 0;
    align-self: stretch;
    overflow: hidden;
  }
  .still img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: #fff;
    background: rgba(0, 0, 0, 0.35);
    opacity: 0;
    transition: opacity 0.2s;
  }
  .card:hover .play,
  .cur .play {
    opacity: 1;
  }
  .tick {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: #000; outline: 1px solid #ffffff70;
    color: #fff;
    animation: pop 0.4s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes pop {
    from {
      transform: scale(0);
    }
  }
  .body {
    display: grid;
    align-content: center;
    gap: 6px;
    min-width: 0;
    padding: 10px 16px;
  }
  .row1,
  .row2 {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    min-width: 0;
  }
  .name {
    min-width: 0;
    font-size: 13.5px;
    font-weight: 800;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .len,
  .when {
    flex: none;
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
  }
  .cur .len,
  .cur .when,
  .cur .ov {
    color: #aaa;
  }
  .cur .pbar {
    background: #ffffff30;
  }
  .cur .pbar i {
    background: #fff;
  }
  .ov {
    min-width: 0;
    font-size: 12.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .star {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12.5px;
    font-weight: 700;
  }
  .star.small {
    font-size: 11.5px;
    color: var(--muted);
  }
  .cur .star.small {
    color: #aaa;
  }
  .ep-sk {
    height: 88px;
    border-radius: 10px;
    margin-bottom: 14px;
  }
  .none {
    padding: 40px 0;
  }

  /* Сезоны у края: подпись, дорожка, номера по шагу 40 px; выбранный - светлая капсула, которая
     перетекает к нему (только transform), дорожка до него светится. */
  .rail {
    align-self: center;
    display: grid;
    justify-items: center;
    gap: 10px;
    max-height: 76%;
    overflow-y: auto;
    padding: 4px 0;
  }
  .cap {
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--faint);
    writing-mode: vertical-rl;
    transform: rotate(180deg);
  }
  .col {
    position: relative;
    display: grid;
    grid-auto-rows: 34px;
    gap: 6px;
    justify-items: center;
  }
  .pill {
    position: absolute;
    top: 0;
    left: 50%;
    width: 44px;
    height: 34px;
    margin-left: -22px;
    border-radius: 12px;
    background: #000; outline: 1px solid #ffffff70;
    box-shadow:
      0 0 0 4px rgba(255, 255, 255, 0.08),
      0 10px 24px -8px rgba(255, 255, 255, 0.45);
    transform: translateY(calc(var(--at) * 40px));
    transition: transform 0.5s cubic-bezier(0.2, 0.9, 0.3, 1.25);
  }
  .rail button {
    position: relative;
    z-index: 1;
    width: 44px;
    height: 34px;
    border: 0;
    border-radius: 12px;
    background: none;
    color: var(--muted);
    font-size: 14px;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    transition:
      color 0.3s,
      transform 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .rail button:hover {
    color: #fff;
    transform: scale(1.12);
  }
  .rail button.on {
    color: #fff;
    transform: none;
  }

  /* Телефон и планшет: кадр сверху, ниже - всё колонкой, сезоны - капсулами. */
  @media (max-width: 1000px) {
    .sh {
      display: block;
      height: auto;
      padding: 0 var(--gutter);
      overflow: visible;
    }
    .bg {
      position: relative;
      inset: auto;
      width: auto;
      height: min(62vw, 440px);
      margin: 0 calc(var(--gutter) * -1);
      overflow: hidden;
    }
    .bg::after {
      background:
        linear-gradient(0deg, #000 0%, rgba(0, 0, 0, 0.4) 45%, transparent 70%),
        linear-gradient(180deg, rgba(0, 0, 0, 0.55), transparent 30%);
    }
    .info {
      margin-top: -96px;
      position: relative;
    }
    .eps {
      overflow: visible;
      padding: 24px 0 0;
      margin: 0;
      mask-image: none;
    }
    .eps .chips {
      display: flex;
      gap: 8px;
      overflow-x: auto;
      margin: 0 calc(var(--gutter) * -1) 6px;
      padding: 2px var(--gutter);
    }
    .cur .card {
      transform: none;
    }
    .cur .card:hover {
      transform: none;
    }
    .card:hover {
      transform: none;
    }
    .rail {
      display: none;
    }
  }
  @media (max-width: 560px) {
    .card {
      grid-template-columns: 120px minmax(0, 1fr);
    }
    .body {
      padding: 8px 12px;
    }
  }

  .tagline { display: -webkit-box; -webkit-line-clamp: 4; line-clamp: 4; -webkit-box-orient: vertical; overflow: hidden; max-width: 54ch; }
  .bg::after { background: linear-gradient(90deg, #000b, #0005 48%, #000c), linear-gradient(0deg, var(--bg), transparent 65%); }
  .ep { border-radius: 16px; overflow: hidden; }
  /* Рамка - только у карточки серии (.cur .card): вторая рамка вокруг строки с галочкой выходила за список. */
  .still { background: #080808; }
  .still-label { position: absolute; inset: 0; display: grid; place-content: center; gap: 4px; font-size: 12px; color: var(--brand); text-align: center; }

  .watch-state{display:inline-flex;align-items:center;gap:5px;font-size:11px;font-weight:700;color:#9ce5b8;}
  .done .still img{opacity:.5;filter:grayscale(.6);}
  .done .tick,.seen.on{background:#fff;color:#000;border-color:#fff;outline:0;}
  .viewing-progress { padding: 10px 0; border: 0; border-radius: 0; background: transparent; max-width: 430px; }
  .progress-seasons { display: none; }
  .progress-head { gap: 18px; }
  .progress-note { margin-top: 6px; color: #888; }
  /* Телефон (slim): верх - MobileHero; здесь прогресс, действия и сезоны вкладками с белой чертой. */
  @media (max-width: 640px) {
    .sh.slim .bg,
    .sh.slim .info > .meta,
    .sh.slim .info > h1,
    .sh.slim .info > .rating,
    .sh.slim .info > .tagline,
    .sh.slim .acts :global(.go),
    .sh.slim .acts :global(.want) {
      display: none;
    }
    .sh.slim .info {
      margin-top: 4px;
    }
    .sh.slim .eps .chips {
      gap: 22px;
      border-bottom: 1px solid rgba(255, 255, 255, 0.12);
      padding-bottom: 0;
    }
    .sh.slim .eps .chip {
      position: relative;
      padding: 8px 0 10px;
      border: 0 !important;
      border-radius: 0;
      background: none !important;
      box-shadow: none !important;
      outline: 0;
      color: rgba(235, 235, 245, 0.6);
      font-size: 16px;
      font-weight: 600;
      white-space: nowrap;
    }
    .sh.slim .eps .chip.on {
      color: #fff;
      background: none;
    }
    .sh.slim .eps .chip.on::after {
      content: '';
      position: absolute;
      left: 0;
      right: 0;
      bottom: -1px;
      height: 2px;
      border-radius: 1px;
      background: #fff;
      animation: tab-in 0.2s ease both;
    }
    @keyframes tab-in {
      from {
        transform: scaleX(0.4);
        opacity: 0;
      }
    }
  }
  .short {
    display: none;
  }
  /* Телефон (slim), как в макете: «Сезон 1 / Сезон 2» вкладками, серии выбранного сезона - лентой
     больших кадров (название и длина под кадром), остальные сезоны скрыты до выбора вкладки. */
  @media (max-width: 640px) {
    .sh.slim .chip .full {
      display: none;
    }
    .sh.slim .chip .short {
      display: inline;
    }
    .sh.slim .sea,
    .sh.slim .other {
      display: none;
    }
    .sh.slim .eps ol {
      display: grid;
      grid-auto-flow: column;
      grid-auto-columns: 78%;
      gap: 10px;
      margin: 14px calc(var(--gutter) * -1) 0;
      padding: 0 var(--gutter) 4px;
      overflow-x: auto;
      scroll-snap-type: x mandatory;
      scroll-padding: 0 var(--gutter);
      overscroll-behavior-x: contain;
    }
    .sh.slim .ep {
      grid-template-columns: minmax(0, 1fr) auto;
      align-items: end;
      scroll-snap-align: start;
      border-radius: 14px;
      box-shadow: none;
      content-visibility: visible;
    }
    .sh.slim .card {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: auto auto;
      min-height: 0;
      border-radius: 14px;
      background: none;
    }
    .sh.slim .still {
      aspect-ratio: 16 / 9;
      border-radius: 14px;
    }
    .sh.slim .ep.cur .still {
      outline: 2px solid #fff;
      outline-offset: -2px;
    }
    .sh.slim .body {
      padding: 8px 2px 0;
    }
    .sh.slim .body .row2 {
      display: none;
    }
    /* Галочка «посмотрел» - рядом с названием, не поверх кнопки серии (две кнопки друг на друге). */
    .sh.slim .seen {
      position: static;
      margin: 0 0 4px 6px;
    }
    .sh.slim .source {
      position: absolute;
      top: 8px;
      right: 8px;
      margin: 0;
    }
  }
  @media (max-width:640px) {
    .sh.slim .chips { display:flex; overflow-x:auto; gap:8px; padding:6px 0 10px; scrollbar-width:thin; }
    .sh.slim .chip { flex:0 0 auto; min-height:44px; }
    .sh.slim .eps ol { grid-auto-flow:row; grid-auto-columns:auto; grid-template-columns:minmax(0,1fr); overflow-x:visible; margin:12px 0 0; padding:0; gap:10px; scroll-snap-type:none; }
    .sh.slim .card { grid-template-columns:120px minmax(0,1fr); grid-template-rows:auto; align-items:center; background:#202022; }
    .sh.slim .still { aspect-ratio:16 / 9; }
    .sh.slim .body { padding:10px; }
    .sh.slim .body .row2 { display:block; }
  }
</style>
