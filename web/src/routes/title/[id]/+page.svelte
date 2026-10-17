<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, errorText, patch, post } from '$lib/api';
  import { rate, toggleList, toggleWatched } from '$lib/cards.svelte';
  import { count, date, kindLabel, runtime, size, votes } from '$lib/fmt';
  import { day, left, percent } from '$lib/library';
  import { session } from '$lib/session.svelte';
  import Reviews from '$lib/social/Reviews.svelte';
  import ShareTitle from '$lib/social/ShareTitle.svelte';
  import StarInput from '$lib/social/StarInput.svelte';
  import type { FileItem, Person, Review, Reviews as ReviewsData, TitleDetail } from '$lib/types';
  import { placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import Lightbox from '$lib/ui/Lightbox.svelte';

  // Страница фильма по референсу: большой кадр, постер и «Смотреть», панель действий
  // (хочу / просмотрено / нравится, оценка, обсудить, отзыв), ниже — лист: оценки и отзывы,
  // серии, актёры, отзывы, похожие, сюжет, детали.
  let t = $state<TitleDetail | null>(null);
  let rv = $state<ReviewsData | null>(null);
  let error = $state('');
  let season = $state<number | null>(null);
  let castAll = $state(false);
  let shot = $state<number | null>(null);
  let sharing = $state(false);
  let reviewsBox = $state<ReturnType<typeof Reviews>>();

  const id = $derived(Number(page.params.id));

  // Фильм и отзывы — вместе: если отзывы придут позже, разделы под ними сдвинутся (CLS).
  $effect(() => {
    const want = id;
    t = null;
    rv = null;
    error = '';
    const empty: ReviewsData = { average: null, votes: 0, total: 0, items: [], canWrite: false };
    Promise.all([
      api<TitleDetail>(`/api/titles/${want}`),
      api<ReviewsData>(`/api/titles/${want}/reviews`).catch(() => empty),
    ]).then(
      ([d, r]) => {
        if (want !== id) return;
        rv = r;
        t = d;
        const cur = d.files.find((f) => f.id === d.next?.fileId);
        season = cur?.season ?? d.seasons[0] ?? null;
        if (page.url.searchParams.get('play') && d.next) goto(`/watch/${d.next.fileId}`, { replaceState: true });
      },
      (e) => (error = errorText(e)),
    );
  });

  const c = $derived(t?.card);
  const people = $derived(t?.people ?? []);
  const crew = $derived(people.filter((p) => p.job === 'director' || p.job === 'creator'));
  const cast = $derived(people.filter((p) => p.job === 'actor'));
  const team = $derived([...crew.map((p) => ({ ...p, role: p.job === 'creator' ? 'Создатель' : 'Режиссёр' })), ...cast]);
  const episodes = $derived((t?.files ?? []).filter((f) => season === null || f.season === season));
  const isSeries = $derived(c?.kind === 'series');
  const started = $derived(!!t?.next && t.next.position > 5);
  const kids = $derived(!!session.me?.user.profile.kids);
  /** Своя оценка — последняя из своих отзывов со звёздами. */
  const mine = $derived(rv?.items.find((r) => r.mine && r.stars) ?? null);

  async function setStars(v: number | null) {
    if (!rv) return;
    try {
      if (mine && v) Object.assign(mine, await patch<Review>(`/api/reviews/${mine.id}`, { body: mine.body, stars: v }));
      else if (v) rv.items = [await post<Review>(`/api/titles/${id}/reviews`, { stars: v }), ...rv.items];
      else return;
      const last = new Map<number, number>();
      for (const r of [...rv.items].reverse()) if (r.stars) last.set(r.author.id, r.stars);
      const all = [...last.values()];
      rv.votes = all.length;
      rv.average = all.length ? all.reduce((a, b) => a + b, 0) / all.length / 2 : null;
      rv.total = rv.items.length;
    } catch {
      /* оценку можно поставить ещё раз */
    }
  }

  function writeReview() {
    document.getElementById('reviews')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setTimeout(() => reviewsBox?.focus(), 400);
  }

  function epLabel(f: FileItem) {
    if (f.episode === null) return f.fileName;
    return f.episodeEnd ? `${f.episode}–${f.episodeEnd}` : String(f.episode);
  }

  function pct(f: FileItem) {
    if (!f.progress) return 0;
    if (f.progress.done) return 100;
    return Math.min(100, (f.progress.position / f.progress.duration) * 100);
  }

  const initials = (p: Person | { name: string }) =>
    p.name
      .split(/\s+/)
      .slice(0, 2)
      .map((w) => w[0])
      .join('');
</script>

{#if error}
  <div class="page empty"><Icon name="info" size={40} /> {error}</div>
{:else if t && c}
  <section class="hero" style:--tint={c.accent ?? '#1c1c1e'}>
    <div class="bg" aria-hidden="true">
      {#if c.backdrop}<img src={c.backdrop} alt="" decoding="async" />{:else}<div class="ph" style:background={placeholder(c.title)}></div>{/if}
    </div>
    <div class="wrap">
      <div class="poster-col">
        <div class="poster" style:view-transition-name="poster-{c.id}" style:background={c.posterLarge ? undefined : placeholder(c.title)}>
          {#if c.posterLarge}<img src={c.posterLarge} alt={c.title} width="460" height="690" decoding="async" />{:else}<span>{c.title}</span>{/if}
        </div>
        {#if t.next}
          <a class="btn white block watch" href="/watch/{t.next.fileId}">
            <Icon name="play" size={16} /> {started ? 'Продолжить' : 'Смотреть'}
          </a>
        {/if}
      </div>

      <div class="info">
        <h1>{c.title}</h1>
        <p class="meta">
          {#if c.year}<span>{c.year}</span>{/if}
          {#if c.runtime}<span>{runtime(c.runtime)}{isSeries ? ' серия' : ''}</span>{/if}
          {#if isSeries}<span>{count(c.seasons || 1, 'сезон', 'сезона', 'сезонов')}</span>{/if}
          {#if c.age}<span class="age">{c.age}</span>{/if}
        </p>
        {#if t.tagline}<p class="tagline">{t.tagline}</p>{/if}
        {#if c.overview}<p class="overview">{c.overview}</p>{/if}
        {#if t.next && (started || (isSeries && t.next.label && c.status === 'watching'))}
          <!-- Где остановились: место в фильме или следующая серия сериала. -->
          <div class="where">
            <span>{t.next.label || 'Вы остановились'}{started ? ` · ${left(t.next.position, t.next.duration)}` : ' · дальше'}</span>
            {#if started}<span class="pbar"><i style:width="{percent(t.next.position, t.next.duration)}%"></i></span>{/if}
          </div>
        {/if}
        {#if c.genres.length}
          <div class="genres">
            {#each c.genres as g (g)}<a class="pill" href="/search?q={encodeURIComponent(g)}">{g}</a>{/each}
          </div>
        {/if}
      </div>

      <aside class="panel" aria-label="Действия">
        <div class="acts">
          <button class:on={c.status === 'watched'} aria-pressed={c.status === 'watched'} onclick={() => toggleWatched(c)}>
            <Icon name="eye" size={22} /><span>{c.status === 'watched' ? 'Просмотрено' : 'Смотрел'}</span>
          </button>
          <button class:on={c.myRating === 2} aria-pressed={c.myRating === 2} onclick={() => rate(c, 2)}>
            <Icon name="heart" size={22} /><span>Нравится</span>
          </button>
          <button
            class:on={c.status === 'want'}
            aria-pressed={c.status === 'want'}
            disabled={c.status === 'watching' || c.status === 'watched'}
            onclick={() => toggleList(c)}
          >
            <Icon name="bookmark" size={22} /><span>Хочу</span>
          </button>
        </div>
        {#if !kids}
          <div class="rate">
            <small>Оценка</small>
            <StarInput value={mine?.stars ?? null} onchange={setStars} size={24} label="Ваша оценка" />
          </div>
          <button class="btn block" onclick={() => (sharing = true)}><Icon name="share" size={15} /> Обсудить с другом</button>
          <button class="btn block" onclick={writeReview}><Icon name="edit" size={15} /> Написать отзыв</button>
        {/if}
        {#if c.status === 'watched' && c.watchedAt}<p class="stub">Просмотрено {day(c.watchedAt)}</p>{/if}
      </aside>
    </div>
  </section>

  <main class="sheet">
    <section class="block summary" aria-labelledby="s-rating">
      <h2 id="s-rating">Оценки и отзывы</h2>
      <div class="score">
        <span class="big"><Icon name="star" size={22} /> {rv?.average ? rv.average.toFixed(1).replace('.', ',') : '—'}</span>
        <span class="muted">{rv ? (rv.votes ? count(rv.votes, 'оценка', 'оценки', 'оценок') : 'оценок пока нет') : ''}</span>
        <span class="grow"></span>
        {#if c.rating}<span class="chip-o"><b class="imdb">IMDb</b> {c.rating.toFixed(1)}</span>{/if}
        {#if t.votes}<span class="chip-o"><Icon name="users" size={14} /> {votes(t.votes)}</span>{/if}
        {#if c.matchPct}<span class="chip-o match">{c.matchPct}% совпадение</span>{/if}
      </div>
    </section>

    {#if isSeries}
      <section class="block" aria-labelledby="s-eps">
        <div class="eps-head">
          <h2 id="s-eps">Серии</h2>
          {#if t.seasons.length > 1}
            <div class="seasons" role="tablist" aria-label="Сезоны">
              {#each t.seasons as s (s)}
                <button class="chip" class:on={season === s} role="tab" aria-selected={season === s} onclick={() => (season = s)}>{s} сезон</button>
              {/each}
            </div>
          {/if}
        </div>
        <ol class="eps">
          {#each episodes as f (f.id)}
            <li>
              <a class="still" href="/watch/{f.id}" style:background={f.still ? undefined : placeholder(c.title + f.id)}>
                {#if f.still}<img src={f.still} alt="" loading="lazy" decoding="async" />{/if}
                <span class="p"><Icon name="play" size={16} /></span>
                {#if pct(f)}<span class="pbar"><i style:width="{pct(f)}%"></i></span>{/if}
              </a>
              <div class="ep">
                <a href="/watch/{f.id}" class="ept"><b>{epLabel(f)}.</b> {f.name ?? `${f.episode ?? ''} серия`}</a>
                {#if f.overview}<p>{f.overview}</p>{/if}
                <div class="epmeta faint">
                  {#if f.duration && f.duration >= 60}<span>{Math.round(f.duration / 60)} мин</span>{/if}
                  {#if f.airDate}<span>{date(f.airDate)}</span>{/if}
                  {#if f.progress?.done}<span class="ok"><Icon name="check" size={13} /> Просмотрено</span>{/if}
                  {#if t.canDownload}<a href="/api/files/{f.id}/download" download class="dl"><Icon name="download" size={13} /> {size(f.size)}</a>{/if}
                </div>
              </div>
            </li>
          {/each}
        </ol>
      </section>
    {/if}

    {#if team.length}
      <section class="block" aria-labelledby="s-cast">
        <h2 id="s-cast">Актёры и съёмочная группа</h2>
        <ul class="cast">
          {#each castAll ? team : team.slice(0, 10) as p, i (i)}
            <li>
              <span class="ava" style:background={p.photo ? undefined : placeholder(p.name)}>
                {#if p.photo}<img src={p.photo} alt="" loading="lazy" decoding="async" width="72" height="72" />{:else}{initials(p)}{/if}
              </span>
              <b>{p.name}</b>
              {#if p.role}<small>{p.role}</small>{/if}
            </li>
          {/each}
        </ul>
        {#if team.length > 10}
          <button class="btn ghost sm" onclick={() => (castAll = !castAll)}>{castAll ? 'Свернуть' : `Все (${team.length})`}</button>
        {/if}
      </section>
    {/if}

    <section class="block" id="reviews" aria-labelledby="s-reviews">
      <h2 id="s-reviews">Отзывы {#if rv?.total}<span class="faint">{rv.total}</span>{/if}</h2>
      {#if rv}
        <Reviews bind:this={reviewsBox} titleId={c.id} bind:data={rv} manage={!!session.me?.manage} />
      {:else}
        <div class="sk-reviews skeleton"></div>
      {/if}
    </section>

    {#if t.similar.length}
      <section class="block" aria-labelledby="s-similar">
        <h2 id="s-similar">Похожие</h2>
        <ul class="similar">
          {#each t.similar.slice(0, 12) as s (s.id)}
            <li>
              <a href="/title/{s.id}" title={s.title} style:background={s.poster ? undefined : placeholder(s.title)}>
                {#if s.poster}<img src={s.poster} alt={s.title} loading="lazy" decoding="async" width="342" height="513" />{:else}<span>{s.title}</span>{/if}
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if c.overview || t.tagline}
      <section class="block" aria-labelledby="s-story">
        <h2 id="s-story">Сюжет</h2>
        <div class="story">
          <b>{c.title}</b>
          <small>{[kindLabel(c.kind), ...c.genres.slice(0, 3)].join(' · ')}</small>
          {#if c.overview}<p>{c.overview}</p>{/if}
        </div>
      </section>
    {/if}

    <section class="block details" aria-labelledby="s-details">
      <h2 id="s-details">Детали</h2>
      <dl>
        {#if t.countries.length}<dt>Страна</dt><dd>{t.countries.join(', ')}</dd>{/if}
        {#if t.languages.length}<dt>Язык</dt><dd>{t.languages.slice(0, 3).join(', ')}</dd>{/if}
        {#if t.releaseDate}<dt>Премьера</dt><dd>{date(t.releaseDate)}</dd>{/if}
        {#if c.originalTitle && c.originalTitle !== c.title}<dt>Оригинал</dt><dd>{c.originalTitle}</dd>{/if}
        {#if t.aka.length}<dt>Также</dt><dd>{t.aka.slice(0, 4).join(', ')}</dd>{/if}
        {#if isSeries}<dt>На сайте</dt><dd>{count(t.files.length, 'серия', 'серии', 'серий')}</dd>{/if}
      </dl>
      <div class="links">
        {#if t.canDownload && !isSeries && t.files[0]}
          <a class="btn sm" href="/api/files/{t.files[0].id}/download" download><Icon name="download" size={14} /> Скачать · {size(t.files[0].size)}</a>
        {/if}
        {#if t.imdbId}
          <a class="btn sm" href="https://www.imdb.com/title/{t.imdbId}/" target="_blank" rel="noopener noreferrer"><b class="imdb">IMDb</b> Открыть</a>
        {/if}
      </div>
      {#if !isSeries && t.files.length > 1}
        <ul class="files">
          {#each t.files as f (f.id)}
            <li>
              <a href="/watch/{f.id}"><Icon name="play" size={14} /> {f.fileName}</a>
              <span class="faint">{size(f.size)}</span>
            </li>
          {/each}
        </ul>
      {/if}
      {#if t.gallery.length}
        <div class="gallery">
          {#each t.gallery.slice(0, 8) as g, k (k)}
            <button onclick={() => (shot = k)} aria-label="Кадр {k + 1}"><img src={g} alt="" loading="lazy" decoding="async" /></button>
          {/each}
        </div>
      {/if}
    </section>
  </main>

  <ShareTitle bind:open={sharing} titleId={c.id} title={c.title} />
  <Lightbox images={t.gallery} bind:index={shot} />
{:else}
  <div class="sk-hero skeleton" aria-busy="true"></div>
{/if}

<style>
  /* ---------- большой кадр ---------- */
  .hero {
    position: relative;
    padding: calc(var(--header) + clamp(120px, 22vw, 260px)) var(--gutter) 34px;
    overflow: hidden;
    isolation: isolate;
  }
  .bg {
    position: absolute;
    inset: 0;
    z-index: -1;
  }
  .bg img,
  .bg .ph {
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 25%;
    /* Медленное приближение кадра — только transform, без сдвигов. */
    animation: kenburns 24s ease-out both;
  }
  .bg::after {
    content: '';
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, var(--bg) 0%, rgba(0, 0, 0, 0.55) 45%, rgba(0, 0, 0, 0.25) 100%),
      linear-gradient(90deg, color-mix(in srgb, var(--tint) 35%, transparent), transparent 60%);
  }
  @keyframes kenburns {
    from {
      transform: scale(1.12);
      opacity: 0;
    }
    8% {
      opacity: 1;
    }
    to {
      transform: scale(1);
    }
  }
  .wrap {
    max-width: 1060px;
    margin: 0 auto;
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr) 220px;
    gap: 32px;
    align-items: end;
  }
  .poster-col {
    display: grid;
    gap: 14px;
    animation: rise 0.6s 0.1s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .poster {
    aspect-ratio: 2 / 3;
    border-radius: 12px;
    overflow: hidden;
    display: grid;
    align-items: end;
    box-shadow: 0 30px 60px -20px rgba(0, 0, 0, 0.9);
  }
  .poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .poster span {
    padding: 16px;
    font-weight: 800;
    font-size: 20px;
  }
  .watch {
    height: 44px;
  }
  .info {
    display: grid;
    gap: 10px;
    align-content: end;
    min-width: 0;
    animation: rise 0.6s 0.18s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  h1 {
    font-size: clamp(30px, 4.6vw, 52px);
    font-weight: 800;
    letter-spacing: -0.03em;
    overflow-wrap: anywhere;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0 10px;
    margin: 0;
    color: #d4d4d8;
    font-size: 14px;
  }
  .meta span + span::before {
    content: '|';
    margin-right: 10px;
    color: rgba(255, 255, 255, 0.35);
  }
  .age {
    padding: 0 6px;
    border: 1px solid rgba(255, 255, 255, 0.55);
    border-radius: 99px;
    font-size: 12px;
  }
  .meta .age::before {
    content: none !important;
  }
  .tagline {
    margin: 4px 0 0;
    color: #e4e4e7;
    font-weight: 600;
  }
  .overview {
    margin: 0;
    color: #d4d4d8;
    font-size: 14px;
    line-height: 1.6;
    display: -webkit-box;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .where {
    display: grid;
    gap: 6px;
    max-width: 420px;
    font-size: 12.5px;
    color: #e4e4e7;
  }
  .pbar {
    height: 4px;
    border-radius: 2px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.2);
  }
  .pbar i {
    display: block;
    height: 100%;
    background: #fff;
  }
  .genres {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }
  .pill {
    padding: 7px 14px;
    border-radius: 99px;
    background: rgba(40, 40, 42, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.12);
    font-size: 13px;
    font-weight: 600;
    transition:
      background 0.15s,
      transform 0.15s;
  }
  .pill:hover {
    background: rgba(60, 60, 64, 0.95);
    transform: translateY(-1px);
  }

  /* Панель действий справа — стекло, как на референсе. */
  .panel {
    display: grid;
    gap: 10px;
    padding: 14px;
    border-radius: 16px;
    background: rgba(28, 28, 30, 0.72);
    backdrop-filter: blur(18px) saturate(140%);
    -webkit-backdrop-filter: blur(18px) saturate(140%);
    border: 1px solid rgba(255, 255, 255, 0.08);
    animation: rise 0.6s 0.26s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .acts {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 4px;
  }
  .acts button {
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 8px 2px;
    border: 0;
    border-radius: 12px;
    background: none;
    color: var(--muted);
    font-size: 11px;
    cursor: pointer;
    transition:
      color 0.15s,
      background 0.15s,
      transform 0.15s cubic-bezier(0.2, 0.9, 0.3, 1.4);
  }
  .acts button:hover:not(:disabled) {
    color: var(--text);
    background: rgba(255, 255, 255, 0.06);
  }
  .acts button:active:not(:disabled) {
    transform: scale(0.9);
  }
  .acts button.on {
    color: #fff;
  }
  .acts button.on :global(svg) {
    fill: currentColor;
    animation: pop 0.35s cubic-bezier(0.2, 0.9, 0.3, 1.6);
  }
  .acts button:nth-child(2).on {
    color: #ff5c7a;
  }
  .acts button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  @keyframes pop {
    40% {
      transform: scale(1.3);
    }
  }
  .rate {
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 8px 0 10px;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
  }
  .rate small {
    color: var(--muted);
    font-size: 11px;
  }
  .panel .btn {
    height: 38px;
    font-size: 13px;
  }
  .stub {
    margin: 0;
    padding: 8px 10px;
    border-radius: 10px;
    background: var(--ticket);
    color: var(--ticket-ink);
    font-family: var(--mono);
    font-size: 12px;
    text-align: center;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(14px);
    }
  }

  /* ---------- лист с разделами ---------- */
  .sheet {
    max-width: 1060px;
    margin: 0 auto;
    padding: 8px clamp(18px, 4vw, 54px) 40px;
    background: #1a1a1c;
    border-radius: 22px;
  }
  .block {
    padding: 28px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  }
  .block:last-child {
    border-bottom: 0;
  }
  h2 {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 17px;
    font-weight: 700;
    margin-bottom: 16px;
  }
  .score {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 14px;
  }
  .big {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 30px;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
  }
  .grow {
    flex: 1;
  }
  .chip-o {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border-radius: 99px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    font-size: 13px;
    font-weight: 600;
  }
  .chip-o.match {
    color: var(--ok);
  }
  .imdb {
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--imdb);
    color: #111;
    font-size: 10.5px;
    font-weight: 800;
  }

  .cast {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
    gap: 18px 12px;
  }
  .cast li {
    display: grid;
    justify-items: center;
    gap: 2px;
    text-align: center;
    min-width: 0;
  }
  .cast .ava {
    width: 72px;
    height: 72px;
    margin-bottom: 6px;
    border-radius: 50%;
    overflow: hidden;
    display: grid;
    place-items: center;
    font-weight: 800;
    font-size: 20px;
    transition: transform 0.2s;
  }
  .cast li:hover .ava {
    transform: scale(1.06);
  }
  .cast .ava img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .cast b {
    font-size: 12.5px;
    font-weight: 600;
    line-height: 1.25;
    overflow-wrap: anywhere;
  }
  .cast small {
    font-size: 11.5px;
    color: var(--faint);
    line-height: 1.25;
    overflow-wrap: anywhere;
  }

  .similar {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    gap: 12px;
  }
  .similar a {
    display: grid;
    align-items: end;
    aspect-ratio: 2 / 3;
    border-radius: 10px;
    overflow: hidden;
    transition:
      transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1),
      box-shadow 0.25s;
  }
  .similar a:hover {
    transform: translateY(-4px) scale(1.02);
    box-shadow: 0 16px 30px -14px #000;
  }
  .similar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .similar span {
    padding: 10px;
    font-size: 13px;
    font-weight: 700;
  }

  .story {
    display: grid;
    gap: 4px;
    padding: 18px 20px;
    border-radius: 14px;
    background: #232325;
  }
  .story small {
    color: var(--faint);
    font-size: 11.5px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .story p {
    margin: 8px 0 0;
    color: #d4d4d8;
    font-size: 14px;
    line-height: 1.6;
  }

  .details dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 8px 18px;
    margin: 0 0 14px;
    font-size: 13.5px;
  }
  .details dt {
    color: var(--muted);
  }
  .details dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .files {
    list-style: none;
    margin: 14px 0 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .files li {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 13px;
  }
  .files a {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 8px;
    margin-top: 16px;
  }
  .gallery button {
    padding: 0;
    border: 0;
    border-radius: 10px;
    overflow: hidden;
    aspect-ratio: 16 / 9;
    cursor: zoom-in;
    background: var(--surface);
  }
  .gallery img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.3s;
  }
  .gallery button:hover img {
    transform: scale(1.05);
  }

  /* ---------- серии ---------- */
  .eps-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 14px;
  }
  .eps-head h2 {
    margin: 0;
  }
  .seasons {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    max-width: 100%;
  }
  .eps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .eps li {
    display: grid;
    grid-template-columns: 180px minmax(0, 1fr);
    gap: 16px;
    padding: 8px;
    border-radius: 14px;
    transition: background 0.15s;
  }
  .eps li:hover {
    background: #232325;
  }
  .still {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: 10px;
    overflow: hidden;
  }
  .still img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .still .p {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 38px;
    height: 38px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.55);
    opacity: 0;
    transform: scale(0.8);
    transition:
      opacity 0.15s,
      transform 0.2s;
  }
  .eps li:hover .p {
    opacity: 1;
    transform: none;
  }
  .still .pbar {
    position: absolute;
    inset: auto 0 0;
    border-radius: 0;
  }
  .ep {
    display: grid;
    align-content: start;
    gap: 4px;
    min-width: 0;
  }
  .ept {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .ep p {
    margin: 0;
    font-size: 13px;
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .epmeta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: 12px;
  }
  .epmeta span,
  .epmeta a {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .ok {
    color: var(--ok);
  }

  .sk-hero {
    height: 70vh;
  }
  .sk-reviews {
    height: 140px;
    border-radius: 14px;
  }

  /* ---------- планшет и телефон ---------- */
  @media (max-width: 980px) {
    .wrap {
      grid-template-columns: 180px minmax(0, 1fr);
    }
    .panel {
      grid-column: 1 / -1;
      grid-template-columns: auto auto 1fr 1fr;
      align-items: center;
    }
    .rate {
      border: 0;
      padding: 0 12px;
      border-left: 1px solid var(--line);
      border-right: 1px solid var(--line);
    }
  }
  @media (max-width: 640px) {
    .hero {
      padding-top: calc(var(--header) + 90px);
      padding-bottom: 20px;
    }
    .wrap {
      grid-template-columns: 110px minmax(0, 1fr);
      gap: 16px 14px;
    }
    .poster-col {
      align-self: start;
    }
    .info {
      align-content: start;
    }
    h1 {
      font-size: 26px;
    }
    .overview {
      grid-column: 1 / -1;
      -webkit-line-clamp: 4;
      line-clamp: 4;
    }
    .panel {
      grid-template-columns: 1fr;
    }
    .rate {
      border: 0;
      border-top: 1px solid var(--line);
      border-bottom: 1px solid var(--line);
      padding: 8px 0 10px;
    }
    .sheet {
      border-radius: 18px 18px 0 0;
      padding-bottom: 32px;
    }
    .eps li {
      grid-template-columns: 120px minmax(0, 1fr);
      gap: 10px;
    }
    .cast {
      grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
    }
    .cast .ava {
      width: 60px;
      height: 60px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .bg img,
    .bg .ph,
    .poster-col,
    .info,
    .panel {
      animation: none;
    }
  }
</style>
