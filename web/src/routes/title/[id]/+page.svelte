<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, errorText, put } from '$lib/api';
  import { count, date, kindLabel, runtime, size, votes } from '$lib/fmt';
  import type { FileItem, Person, TitleDetail } from '$lib/types';
  import { placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import Lightbox from '$lib/ui/Lightbox.svelte';
  import Poster from '$lib/ui/Poster.svelte';
  import Stars from '$lib/ui/Stars.svelte';

  let t = $state<TitleDetail | null>(null);
  let error = $state('');
  let season = $state<number | null>(null);
  let castAll = $state(false);
  let shot = $state<number | null>(null);
  let copied = $state(false);

  const id = $derived(Number(page.params.id));

  $effect(() => {
    const want = id;
    t = null;
    error = '';
    api<TitleDetail>(`/api/titles/${want}`).then(
      (d) => {
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
  const directors = $derived(people.filter((p) => p.job === 'director' || p.job === 'creator'));
  const writers = $derived(people.filter((p) => p.job === 'writer'));
  const cast = $derived(people.filter((p) => p.job === 'actor'));
  const episodes = $derived((t?.files ?? []).filter((f) => season === null || f.season === season));
  const isSeries = $derived(c?.kind === 'series');
  const watchLabel = $derived(
    t?.next && t.next.position > 5 ? 'Продолжить' : isSeries && t?.next?.label ? 'Смотреть' : 'Смотреть',
  );

  async function toggleList() {
    if (!t) return;
    t.inList = !t.inList;
    await put(`/api/list/${t.card.id}`, { value: t.inList }).catch(() => t && (t.inList = !t.inList));
  }

  async function share() {
    try {
      await navigator.clipboard.writeText(location.href);
      copied = true;
      setTimeout(() => (copied = false), 1800);
    } catch {
      /* буфер обмена недоступен (http) — ничего страшного */
    }
  }

  function names(list: Person[]) {
    return list.map((p) => p.name).join(', ');
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
</script>

{#if error}
  <div class="page empty"><Icon name="info" size={40} /> {error}</div>
{:else if t && c}
  <section class="hero">
    <div class="bg" style:background-image={c.backdrop ? `url(${c.backdrop})` : placeholder(c.title)}></div>
    <div class="shade"></div>
    {#if t.next}
      <a class="bigplay" href="/watch/{t.next.fileId}" aria-label="Смотреть"><Icon name="play" size={34} /></a>
    {/if}
    <div class="bar">
      <h1>{c.title} {#if c.year}<span class="y">({c.year})</span>{/if}</h1>
      <div class="chips">
        {#if c.rating}<span class="chip imdbchip"><span class="imdb">IMDb</span> {c.rating.toFixed(1)}</span>{/if}
        <button class="chip" onclick={toggleList}>
          <Icon name={t.inList ? 'check' : 'plus'} size={15} />
          {t.inList ? 'В моём списке' : 'Смотреть позже'}
        </button>
        <button class="chip" onclick={share}><Icon name={copied ? 'check' : 'share'} size={15} /> {copied ? 'Ссылка скопирована' : 'Поделиться'}</button>
      </div>
    </div>
  </section>

  <div class="body">
    <aside class="left">
      <div class="poster" style:background={c.poster ? undefined : placeholder(c.title)}>
        {#if c.poster}<img src={c.poster} alt={c.title} />{:else}<span>{c.title}</span>{/if}
      </div>
      {#if t.next}
        <a class="btn primary block" href="/watch/{t.next.fileId}"><Icon name="play" size={16} /> {watchLabel}</a>
        {#if t.next.label}<div class="nextlabel muted">{t.next.label}</div>{/if}
      {/if}
      <button class="btn block" onclick={toggleList}>
        <Icon name={t.inList ? 'check' : 'plus'} size={16} />
        {t.inList ? 'В моём списке' : 'В мой список'}
      </button>
      {#if t.canDownload && !isSeries && t.files[0]}
        <a class="btn block" href="/api/files/{t.files[0].id}/download" download><Icon name="download" size={16} /> Скачать · {size(t.files[0].size)}</a>
      {/if}
    </aside>

    <div class="main">
      <div class="titleline">
        <h2>{c.title}</h2>
        {#if c.rating}<span class="imdb">IMDb <b>{c.rating.toFixed(1)}</b></span>{/if}
      </div>
      {#if t.tagline}<p class="tagline">{t.tagline}</p>{/if}
      <div class="meta">
        {#if t.votes}<span><Icon name="eye" size={15} /> {votes(t.votes)}</span>{/if}
        {#if c.rating}<span><Stars rating={c.rating} size={13} /></span>{/if}
        {#if c.age}<span class="age">{c.age}</span>{/if}
        {#if c.runtime}<span>{runtime(c.runtime)}{isSeries ? ' серия' : ''}</span>{/if}
        {#if c.genres.length}<span>{c.genres.join(', ')}</span>{/if}
        <span>{isSeries ? count(c.seasons || 1, 'сезон', 'сезона', 'сезонов') : kindLabel(c.kind)}</span>
        {#if c.year}<span>{c.year}</span>{/if}
      </div>

      <div class="cols">
        <div>
          <h3>Детали</h3>
          <dl>
            {#if directors.length}<dt>{isSeries ? 'Создатели' : 'Режиссёр'}</dt><dd>{names(directors)}</dd>{/if}
            {#if writers.length}<dt>Сценарий</dt><dd>{names(writers)}</dd>{/if}
            {#if t.countries.length}<dt>Страна</dt><dd>{t.countries.join(', ')}</dd>{/if}
            {#if t.languages.length}<dt>Язык</dt><dd>{t.languages.slice(0, 3).join(', ')}</dd>{/if}
            {#if t.releaseDate}<dt>Премьера</dt><dd>{date(t.releaseDate)}</dd>{/if}
            {#if c.originalTitle && c.originalTitle !== c.title}<dt>Оригинал</dt><dd>{c.originalTitle}</dd>{/if}
            {#if t.aka.length}<dt>Также известен</dt><dd>{t.aka.join(', ')}</dd>{/if}
            {#if isSeries}<dt>На сайте</dt><dd>{count(t.files.length, 'серия', 'серии', 'серий')}</dd>{/if}
          </dl>
        </div>
        {#if cast.length}
          <div>
            <h3>В ролях</h3>
            <ul class="cast">
              {#each castAll ? cast : cast.slice(0, 5) as p (p.name + p.role)}
                <li>
                  <span class="ava" style:background={p.photo ? undefined : placeholder(p.name)}>
                    {#if p.photo}<img src={p.photo} alt="" loading="lazy" />{:else}{p.name[0]}{/if}
                  </span>
                  <span class="pn">{p.name}{#if p.role}<small>{p.role}</small>{/if}</span>
                </li>
              {/each}
            </ul>
            {#if cast.length > 5}
              <button class="more" onclick={() => (castAll = !castAll)}>
                <Icon name={castAll ? 'back' : 'plus'} size={14} /> {castAll ? 'Свернуть' : 'Показать всех'}
              </button>
            {/if}
          </div>
        {/if}
      </div>

      {#if c.overview}
        <h3>Сюжет</h3>
        <p class="story">{c.overview}</p>
      {/if}

      {#if isSeries}
        <div class="eps-head">
          <h3>Серии</h3>
          {#if t.seasons.length > 1}
            <div class="seasons" role="tablist">
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
                {#if f.still}<img src={f.still} alt="" loading="lazy" />{/if}
                <span class="p"><Icon name="play" size={16} /></span>
                {#if pct(f)}<span class="pbar"><i style:width="{pct(f)}%"></i></span>{/if}
              </a>
              <div class="ep">
                <div class="eptitle">
                  <a href="/watch/{f.id}"><b>{epLabel(f)}.</b> {f.name ?? `${f.episode ?? ''} серия`}</a>
                  {#if f.duration && f.duration >= 60}<span class="faint">{Math.round(f.duration / 60)} мин</span>{/if}
                </div>
                {#if f.overview}<p>{f.overview}</p>{/if}
                <div class="epmeta faint">
                  {#if f.airDate}<span>{date(f.airDate)}</span>{/if}
                  {#if f.progress?.done}<span class="ok"><Icon name="check" size={13} /> Просмотрено</span>{/if}
                  {#if t.canDownload}<a href="/api/files/{f.id}/download" download class="dl"><Icon name="download" size={13} /> {size(f.size)}</a>{/if}
                </div>
              </div>
            </li>
          {/each}
        </ol>
      {:else if t.files.length > 1}
        <h3>Файлы</h3>
        <ul class="files">
          {#each t.files as f (f.id)}
            <li>
              <a href="/watch/{f.id}"><Icon name="play" size={14} /> {f.fileName}</a>
              <span class="faint">{size(f.size)}</span>
              {#if t.canDownload}<a href="/api/files/{f.id}/download" download aria-label="Скачать"><Icon name="download" size={16} /></a>{/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <aside class="right">
      {#if t.gallery.length}
        <h3>Галерея</h3>
        <div class="gallery">
          {#each t.gallery.slice(0, 7) as g, k (g)}
            <button class:first={k === 0} onclick={() => (shot = k)} aria-label="Кадр {k + 1}">
              <img src={g} alt="" loading="lazy" />
              {#if k === 0 && t.next}<span class="p"><Icon name="play" size={18} /></span>{/if}
            </button>
          {/each}
        </div>
      {/if}
      {#if t.imdbId}
        <a class="btn block sm imdblink" href="https://www.imdb.com/title/{t.imdbId}/" target="_blank" rel="noopener noreferrer">
          <span class="imdb">IMDb</span> Открыть на IMDb
        </a>
      {/if}
    </aside>
  </div>

  {#if t.similar.length}
    <section class="similar">
      <h2 class="section-title"><Icon name="film" /> Похожие</h2>
      <div class="grid">
        {#each t.similar.slice(0, 12) as s (s.id)}
          <Poster card={s} />
        {/each}
      </div>
    </section>
  {/if}

  <Lightbox images={t.gallery} bind:index={shot} />
{:else}
  <div class="sk skeleton"></div>
{/if}

<style>
  .hero {
    position: relative;
    height: min(64vh, 640px);
    min-height: 380px;
    display: flex;
    align-items: flex-end;
    overflow: hidden;
  }
  .bg {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center 30%;
    animation: zoom 1.4s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes zoom {
    from {
      transform: scale(1.08);
      opacity: 0;
    }
  }
  .shade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, var(--bg) 0%, rgba(11, 10, 26, 0.35) 45%, rgba(40, 24, 110, 0.25) 100%),
      radial-gradient(ellipse at center, transparent 40%, rgba(11, 10, 26, 0.5) 100%);
  }
  .bigplay {
    position: absolute;
    left: 50%;
    top: 46%;
    transform: translate(-50%, -50%);
    width: 84px;
    height: 84px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    padding-left: 5px;
    background: #fff;
    color: var(--accent);
    box-shadow:
      0 0 0 10px rgba(255, 255, 255, 0.14),
      0 20px 50px -10px rgba(0, 0, 0, 0.6);
    transition: transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .bigplay:hover {
    transform: translate(-50%, -50%) scale(1.08);
  }
  .bar {
    position: relative;
    width: 100%;
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 14px;
    padding: 0 var(--gutter) 18px;
    border-bottom: 1px solid var(--line);
  }
  h1 {
    font-size: clamp(26px, 3.4vw, 40px);
    font-weight: 700;
    text-shadow: 0 4px 24px rgba(0, 0, 0, 0.5);
  }
  .y {
    font-weight: 400;
    color: #d6d3ea;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chips .chip {
    background: rgba(11, 10, 26, 0.5);
    backdrop-filter: blur(10px);
  }
  .imdbchip .imdb {
    height: 18px;
    font-size: 10px;
  }

  .body {
    display: grid;
    grid-template-columns: 230px minmax(0, 1fr) 300px;
    gap: 36px;
    padding: 30px var(--gutter) 0;
  }
  .left {
    display: grid;
    align-content: start;
    gap: 10px;
    position: relative;
  }
  .poster {
    aspect-ratio: 2 / 3;
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 30px 60px -20px rgba(0, 0, 0, 0.9);
    margin-bottom: 8px;
    display: grid;
    align-items: end;
    padding: 0;
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
  .nextlabel {
    font-size: 12px;
    text-align: center;
    margin-top: -4px;
  }
  .titleline {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  h2 {
    font-size: 26px;
  }
  .tagline {
    margin: 6px 0 0;
    color: var(--muted);
    font-style: italic;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 0;
    margin: 14px 0 6px;
    font-size: 13px;
    color: var(--muted);
  }
  .meta > span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    border-left: 1px solid var(--line);
  }
  .meta > span:first-child {
    padding-left: 0;
    border-left: 0;
  }
  .age {
    border: 1px solid rgba(255, 255, 255, 0.3) !important;
    border-radius: 4px;
    padding: 0 6px !important;
    margin: 0 12px;
  }
  h3 {
    font-size: 17px;
    margin: 28px 0 12px;
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 28px;
    border-top: 1px solid var(--line);
    margin-top: 18px;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px 16px;
    margin: 0;
    font-size: 14px;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
  }
  .cast {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .cast li {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 14px;
  }
  .ava {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    overflow: hidden;
    flex: none;
    display: grid;
    place-items: center;
    font-weight: 700;
    font-size: 13px;
  }
  .ava img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .pn {
    display: grid;
    line-height: 1.25;
  }
  .pn small {
    color: var(--faint);
    font-size: 12px;
  }
  .more {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 12px;
    border: 0;
    background: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 13px;
    padding: 0;
  }
  .more:hover {
    color: var(--text);
  }
  .story {
    margin: 0;
    color: #d3d0e6;
    max-width: 75ch;
  }

  .eps-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 28px;
  }
  .eps-head h3 {
    margin: 0;
  }
  .seasons {
    display: flex;
    gap: 6px;
    overflow-x: auto;
  }
  .eps {
    list-style: none;
    padding: 0;
    margin: 16px 0 0;
    display: grid;
    gap: 8px;
  }
  .eps li {
    display: grid;
    grid-template-columns: 200px 1fr;
    gap: 16px;
    padding: 10px;
    border-radius: var(--radius);
    transition: background 0.15s;
  }
  .eps li:hover {
    background: var(--surface);
  }
  .still {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: 8px;
    overflow: hidden;
  }
  .still img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .p {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    padding-left: 2px;
    background: rgba(11, 10, 26, 0.6);
    border: 1.5px solid rgba(255, 255, 255, 0.8);
    opacity: 0;
    transition: opacity 0.2s;
  }
  .eps li:hover .p,
  .gallery button:hover .p,
  .gallery .first .p {
    opacity: 1;
  }
  .pbar {
    position: absolute;
    inset: auto 0 0;
    height: 3px;
    background: rgba(255, 255, 255, 0.2);
  }
  .pbar i {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .eptitle {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 15px;
  }
  .ep p {
    margin: 4px 0 0;
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
    gap: 14px;
    margin-top: 6px;
    font-size: 12px;
  }
  .epmeta span,
  .dl {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .ok {
    color: var(--ok);
  }
  .dl:hover {
    color: var(--text);
  }
  .files {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 6px;
  }
  .files li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    font-size: 14px;
  }
  .files li a:first-child {
    flex: 1;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .right h3 {
    margin-top: 0;
  }
  .gallery {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }
  .gallery button {
    position: relative;
    padding: 0;
    border: 0;
    border-radius: 8px;
    overflow: hidden;
    aspect-ratio: 16 / 10;
    cursor: zoom-in;
    background: var(--surface);
  }
  .gallery .first {
    grid-column: 1 / -1;
  }
  .gallery img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.3s;
  }
  .gallery button:hover img {
    transform: scale(1.06);
  }
  .imdblink {
    margin-top: 14px;
  }
  .imdblink .imdb {
    height: 18px;
    font-size: 10px;
  }
  .similar {
    padding: 48px var(--gutter) 0;
  }
  .sk {
    height: 64vh;
  }

  @media (max-width: 1180px) {
    .body {
      grid-template-columns: 210px minmax(0, 1fr);
    }
    .right {
      grid-column: 1 / -1;
    }
    .gallery {
      grid-template-columns: repeat(4, 1fr);
    }
    .gallery .first {
      grid-column: span 2;
      grid-row: span 2;
      aspect-ratio: auto;
    }
  }
  @media (max-width: 760px) {
    .body {
      grid-template-columns: 1fr;
      gap: 18px;
    }
    .left {
      grid-template-columns: 120px 1fr;
      align-items: end;
    }
    .poster {
      grid-row: span 4;
      margin: 0;
    }
    .cols {
      grid-template-columns: 1fr;
      gap: 0;
    }
    .eps li {
      grid-template-columns: 130px 1fr;
      gap: 12px;
      padding: 8px 0;
    }
    .bigplay {
      width: 68px;
      height: 68px;
    }
    .gallery {
      grid-template-columns: repeat(3, 1fr);
    }
  }
</style>
