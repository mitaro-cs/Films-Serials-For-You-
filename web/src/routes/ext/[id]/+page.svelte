<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, del, errorText, post, put } from '$lib/api';
  import { clock, date, kindLabel, runtime, votes } from '$lib/fmt';
  import { session } from '$lib/session.svelte';
  import { extChanged, extSheet, remember } from '$lib/discover/sheet.svelte';
  import LinkCard from '$lib/links/LinkCard.svelte';
  import LinkFinder from '$lib/links/LinkFinder.svelte';
  import NewList from '$lib/social/NewList.svelte';
  import { isFavorite, toggleFavorite } from '$lib/social/favorites';
  import type { ExtCard, ExtDetail, FoundVideo } from '$lib/types';
  import { placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import Lightbox from '$lib/ui/Lightbox.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  // Страница фильма или сериала из ленты IMDb — как у фильма с сервиса: большой кадр, постер,
  // панель «Буду смотреть / Просмотрел / Любимый», ниже лист: смотреть онлайн (Rutube, VK,
  // свои ссылки), актёры, похожие (тоже страницы), сюжет, детали и кадры. Есть у нас — «Смотреть
  // на Nova». Карточка, по которой нажали, показывается сразу, описание приходит следом.
  let d = $state<ExtDetail | null>(null);
  let error = $state('');
  let finding = $state(false);
  let creating = $state(false);
  let lists = $state(false);
  let castAll = $state(false);
  let shot = $state<number | null>(null);
  // Сам фильм на Rutube/VK Видео: нашёлся — смотрим там, просить хоста незачем.
  let online = $state<FoundVideo[] | null>(null);

  const id = $derived(page.params.id ?? '');

  $effect(() => {
    const want = id;
    d = null;
    error = '';
    lists = false;
    castAll = false;
    online = null;
    load(want);
  });

  function load(want: string) {
    api<ExtDetail>(`/api/discover/title/${want}`).then(
      (r) => {
        if (want !== id) return;
        d = r;
        if (!r.card.local && !r.links.length && online === null) lookOnline(r);
      },
      (x) => {
        if (want === id) error = errorText(x);
      },
    );
  }

  const seed = $derived(extSheet.seed?.id === id ? extSheet.seed : null);
  const c = $derived<ExtCard | null>(d?.card ?? seed);
  const isSeries = $derived(c?.kind === 'series');
  const people = $derived(d?.people ?? []);
  const team = $derived([
    ...people
      .filter((p) => p.job === 'director' || p.job === 'creator')
      .map((p) => ({ ...p, role: p.job === 'creator' ? 'Создатель' : 'Режиссёр' })),
    ...people.filter((p) => p.job === 'actor'),
  ]);

  const norm = (s: string) =>
    s
      .toLowerCase()
      .replace(/ё/g, 'е')
      .replace(/[^\p{L}\p{N}]+/gu, ' ')
      .trim();

  /** Полный фильм (от 40 мин) или серия (от 15 мин), в названии которых есть название фильма. */
  async function lookOnline(r: ExtDetail) {
    const q = `${r.card.title}${r.card.year ? ` ${r.card.year}` : ''}`;
    const min = r.card.kind === 'series' ? 15 * 60 : 40 * 60;
    const found = await api<FoundVideo[]>(`/api/links/search?min=${min}&q=${encodeURIComponent(q)}`).catch(() => [] as FoundVideo[]);
    if (r.card.id !== id) return;
    const names = [r.card.title, r.card.original].filter((n): n is string => !!n).map(norm);
    online = found.filter((f) => names.some((n) => norm(f.label).includes(n))).slice(0, 8);
  }

  async function playFound(f: FoundVideo) {
    if (!d) return;
    try {
      const r = await post<{ id: number }>('/api/links', {
        url: f.url,
        extId: d.card.id,
        label: f.label,
        thumb: f.thumbUrl,
        duration: f.duration,
      });
      goto(`/watch/link/${r.id}`);
    } catch (x) {
      toast(errorText(x));
    }
  }

  // «Хочу на Nova» — только когда фильма нет ни у нас, ни по ссылкам, ни на Rutube/VK.
  const canWish = $derived(!!d && !d.card.local && !d.links.length && online !== null && !online.length);
  // Главная кнопка под постером: у нас — «Смотреть на Nova», иначе — первое видео онлайн.
  const first = $derived(d?.links[0] ?? null);
  const firstFound = $derived(online?.[0] ?? null);

  async function mark(status: 'want' | 'watched') {
    if (!d) return;
    const before = d.card.mark;
    const next = before === status ? null : status;
    d.card.mark = next;
    try {
      await put(`/api/discover/title/${d.card.id}/mark`, { status: next });
      extChanged();
    } catch {
      d.card.mark = before;
    }
  }

  async function wish() {
    if (!d) return;
    const next = !d.card.wished;
    d.card.wished = next;
    try {
      await put(`/api/discover/title/${d.card.id}/wish`, { value: next });
      toast(next ? 'Хост увидит просьбу добавить этот фильм' : 'Просьба отменена');
    } catch {
      d.card.wished = !next;
    }
  }

  async function toggleList(l: { id: number; name: string; has: boolean }) {
    if (!d) return;
    l.has = !l.has;
    try {
      await put(`/api/lists/${l.id}/ext/${d.card.id}`, { value: l.has });
      toast(l.has ? `Добавили в «${l.name}»` : `Убрали из «${l.name}»`);
      extChanged();
    } catch (x) {
      l.has = !l.has;
      toast(errorText(x));
    }
  }

  async function created(list: number) {
    if (!d) return;
    await put(`/api/lists/${list}/ext/${d.card.id}`, { value: true }).catch(() => {});
    extChanged();
    load(d.card.id);
  }

  async function removeLink(link: number) {
    if (!d) return;
    await del(`/api/links/${link}`).catch((x) => toast(errorText(x)));
    load(d.card.id);
  }

  const initials = (name: string) =>
    name
      .split(/\s+/)
      .slice(0, 2)
      .map((w) => w[0])
      .join('');
  const me = $derived(session.me?.user.profile?.id);
</script>

{#if error}
  <div class="page empty"><Icon name="info" size={40} /> {error}</div>
{:else if c}
  <section class="hero">
    <div class="bg" aria-hidden="true">
      {#key d?.backdrop}
        {#if d?.backdrop}<img use:fadeIn src={d.backdrop} alt="" decoding="async" />{:else}<div class="ph" style:background={placeholder(c.title)}></div>{/if}
      {/key}
    </div>
    <div class="wrap">
      <div class="poster-col">
        <div class="poster" style:background={c.poster || d?.posterLarge ? undefined : placeholder(c.title)}>
          {#if d?.posterLarge || c.poster}
            <img use:fadeIn src={d?.posterLarge ?? c.poster} alt={c.title} width="460" height="690" decoding="async" />
          {:else}<span>{c.title}</span>{/if}
        </div>
        {#if c.local}
          <a class="btn white block watch" href="/title/{c.local}"><Icon name="play" size={16} /> Смотреть на Nova</a>
        {:else if first}
          <a class="btn white block watch" href="/watch/link/{first.id}"><Icon name="play" size={16} /> Смотреть</a>
        {:else if firstFound}
          <button class="btn white block watch" onclick={() => playFound(firstFound)}><Icon name="play" size={16} /> Смотреть</button>
        {:else}
          <button class="btn white block watch" disabled={!d} onclick={() => (finding = true)}><Icon name="search" size={16} /> Найти видео</button>
        {/if}
      </div>

      <div class="info">
        <h1>{c.title}</h1>
        <p class="meta">
          {#if c.year}<span>{c.year}</span>{/if}
          <span>{kindLabel(c.kind)}</span>
          {#if d?.runtime}<span>{runtime(d.runtime)}{isSeries ? ' серия' : ''}</span>{/if}
          {#if d?.age}<span class="age">{d.age}</span>{/if}
        </p>
        {#if c.original}<p class="orig">{c.original}</p>{/if}
        {#if d?.tagline}<p class="tagline">{d.tagline}</p>{/if}
        {#if d?.overview}<p class="overview">{d.overview}</p>{:else if !d}<p class="overview ph-text skeleton"></p>{/if}
        {#if c.genres.length}
          <div class="genres">
            {#each c.genres as g (g)}<span class="pill">{g}</span>{/each}
          </div>
        {/if}
      </div>

      <aside class="panel" aria-label="Действия">
        <div class="box">
          <div class="acts" class:four={canWish || c.wished}>
            <button class:on={c.mark === 'want'} aria-pressed={c.mark === 'want'} disabled={!d} onclick={() => mark('want')}>
              <Icon name="bookmark" size={22} /><span>Буду смотреть</span>
            </button>
            <button class:on={c.mark === 'watched'} aria-pressed={c.mark === 'watched'} disabled={!d} onclick={() => mark('watched')}>
              <Icon name="eye" size={22} /><span>Просмотрел</span>
            </button>
            <button class:on={isFavorite(`x:${c.id}`)} aria-pressed={isFavorite(`x:${c.id}`)} disabled={!d} onclick={() => toggleFavorite(`x:${c.id}`)}>
              <Icon name="star" size={22} /><span>Любимый</span>
            </button>
            {#if canWish || c.wished}
              <button class:on={c.wished} aria-pressed={c.wished} disabled={!d} onclick={wish}>
                <Icon name="mail" size={22} /><span>{c.wished ? 'Попросили' : 'Хочу на Nova'}</span>
              </button>
            {/if}
          </div>
          {#if c.rating}
            <div class="rate">
              <small>IMDb</small>
              <b><Icon name="star" size={16} /> {c.rating.toFixed(1)}</b>
              {#if d?.votes}<small>{votes(d.votes)} оценок</small>{/if}
            </div>
          {/if}
        </div>
        <button class="btn block" aria-expanded={lists} disabled={!d} onclick={() => (lists = !lists)}><Icon name="plus" size={15} /> В список</button>
        {#if lists && d}
          <div class="lists" role="group" aria-label="Мои списки">
            {#each d.lists as l (l.id)}
              <button class="li" class:on={l.has} aria-pressed={l.has} onclick={() => toggleList(l)}>
                <span class="tick"><Icon name="check" size={13} stroke={3} /></span>{l.name}
              </button>
            {/each}
            <button class="li new" onclick={() => (creating = true)}><Icon name="plus" size={14} /> Новый список</button>
          </div>
        {/if}
        <button class="btn block" disabled={!d} onclick={() => (finding = true)}><Icon name="link" size={15} /> Добавить видео</button>
        <a class="btn block" href="https://www.imdb.com/title/{c.id}/" target="_blank" rel="noopener noreferrer"><b class="imdb">IMDb</b> Открыть</a>
      </aside>
    </div>
  </section>

  <main class="sheet">
    <section class="block" aria-labelledby="s-online">
      <div class="lhead">
        <h2 id="s-online">Смотреть онлайн {#if d?.links.length}<span class="faint">{d.links.length}</span>{/if}</h2>
        <button class="btn sm ghost" disabled={!d} onclick={() => (finding = true)}><Icon name="search" size={14} /> Найти видео</button>
      </div>
      {#if d?.links.length}
        <div class="vlinks">
          {#each d.links as l (l.id)}
            <LinkCard link={l} onremove={session.me?.manage || l.addedBy === me ? () => removeLink(l.id) : undefined} />
          {/each}
        </div>
      {/if}
      {#if online?.length}
        {#if d?.links.length}<h3 class="sub">Ещё на Rutube и VK Видео</h3>{/if}
        <ul class="vlinks found">
          {#each online as f (f.url)}
            <li>
              <button onclick={() => playFound(f)} aria-label="Смотреть: {f.label}">
                <span class="art">
                  {#if f.thumb}<img use:fadeIn src={f.thumb} alt="" loading="lazy" decoding="async" />{/if}
                  <span class="p"><Icon name="play" size={18} /></span>
                  <span class="site">{f.site === 'vk' ? 'VK Видео' : 'Rutube'}</span>
                  {#if f.duration}<span class="dur">{clock(f.duration)}</span>{/if}
                </span>
                <span class="name">{f.label}</span>
              </button>
            </li>
          {/each}
        </ul>
      {:else if !d || (!c.local && !d.links.length && online === null)}
        <p class="muted none">Ищем на Rutube и VK Видео…</p>
      {:else if !d.links.length}
        <p class="muted none">
          {c.local ? 'Фильм есть на Nova — смотрите там.' : 'Не нашли сами.'} Найдите видео на Rutube или вставьте ссылку — оно откроется прямо в плеере Nova.
        </p>
      {/if}
    </section>

    {#if team.length}
      <section class="block" aria-labelledby="s-cast">
        <h2 id="s-cast">Актёры и съёмочная группа</h2>
        <ul class="cast">
          {#each castAll ? team : team.slice(0, 12) as p, i (i)}
            <li>
              <span class="ava" style:background={p.photo ? undefined : placeholder(p.name)}>
                {#if p.photo}<img use:fadeIn src={p.photo} alt="" loading="lazy" decoding="async" width="72" height="72" />{:else}{initials(p.name)}{/if}
              </span>
              <b>{p.name}</b>
              {#if p.role}<small>{p.role}</small>{/if}
            </li>
          {/each}
        </ul>
        {#if team.length > 12}
          <button class="btn ghost sm" onclick={() => (castAll = !castAll)}>{castAll ? 'Свернуть' : `Все (${team.length})`}</button>
        {/if}
      </section>
    {/if}

    {#if d?.similar.length}
      <section class="block" aria-labelledby="s-similar">
        <h2 id="s-similar">Похожие</h2>
        <ul class="similar">
          {#each d.similar as s (s.id)}
            <li>
              <a href="/ext/{s.id}" title={s.title} onclick={() => remember(s)} style:background={s.poster ? undefined : placeholder(s.title)}>
                {#if s.poster}<img use:fadeIn src={s.poster} alt={s.title} loading="lazy" decoding="async" width="342" height="513" />{:else}<span>{s.title}</span>{/if}
                {#if s.local}<em>Есть у нас</em>{/if}
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if d?.overview}
      <section class="block" aria-labelledby="s-story">
        <h2 id="s-story">Сюжет</h2>
        <div class="story">
          <b>{c.title}</b>
          <small>{[kindLabel(c.kind), ...c.genres.slice(0, 3)].join(' · ')}</small>
          <p>{d.overview}</p>
        </div>
      </section>
    {/if}

    {#if d}
      <section class="block details" aria-labelledby="s-details">
        <h2 id="s-details">Детали</h2>
        <dl>
          {#if d.countries.length}<dt>Страна</dt><dd>{d.countries.join(', ')}</dd>{/if}
          {#if d.languages.length}<dt>Язык</dt><dd>{d.languages.slice(0, 3).join(', ')}</dd>{/if}
          {#if d.releaseDate}<dt>Премьера</dt><dd>{date(d.releaseDate)}</dd>{/if}
          {#if c.original}<dt>Оригинал</dt><dd>{c.original}</dd>{/if}
          {#if d.aka.length}<dt>Также</dt><dd>{d.aka.slice(0, 4).join(', ')}</dd>{/if}
          {#if d.votes}<dt>Оценок IMDb</dt><dd>{d.votes.toLocaleString('ru-RU')}</dd>{/if}
        </dl>
        {#if d.gallery.length}
          <div class="gallery">
            {#each d.gallery as g, k (k)}
              <button onclick={() => (shot = k)} aria-label="Кадр {k + 1}"><img use:fadeIn src={g} alt="" loading="lazy" decoding="async" /></button>
            {/each}
          </div>
        {/if}
      </section>
    {/if}
  </main>

  {#if d}<Lightbox images={d.gallery} bind:index={shot} />{/if}
{:else}
  <div class="sk-hero skeleton" aria-busy="true"></div>
{/if}

<NewList bind:open={creating} oncreated={created} />
{#if d}
  <LinkFinder bind:open={finding} query="{d.card.title}{d.card.year ? ` ${d.card.year}` : ''}" extId={d.card.id} onadded={() => d && load(d.card.id)} />
{/if}

<style>
  /* Та же раскладка, что у страницы фильма с сервиса (title/[id]). */
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
    animation: kenburns 1.4s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .bg::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, var(--bg) 0%, rgba(0, 0, 0, 0.55) 45%, rgba(0, 0, 0, 0.25) 100%);
  }
  @keyframes kenburns {
    from {
      opacity: 0;
      transform: scale(1.06);
    }
  }
  .wrap {
    max-width: 1440px;
    margin: 0 auto;
    display: grid;
    grid-template-columns: clamp(200px, 17vw, 260px) minmax(0, 1fr) clamp(220px, 18vw, 270px);
    gap: 32px;
    align-items: end;
  }
  .poster-col {
    display: grid;
    gap: 14px;
    animation: rise 0.45s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .poster {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: 12px;
    overflow: hidden;
    display: grid;
    align-items: end;
    box-shadow: 0 30px 60px -20px rgba(0, 0, 0, 0.9);
    transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .poster:hover {
    transform: translateY(-4px) scale(1.02);
  }
  .poster img {
    position: absolute;
    inset: 0;
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
    animation: rise 0.45s 0.06s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  h1 {
    font-size: clamp(28px, 3.2vw, 44px);
    line-height: 1.08;
    font-weight: 800;
    letter-spacing: -0.03em;
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
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
  .orig {
    margin: 0;
    color: var(--muted);
    font-size: 14px;
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
  .ph-text {
    height: 66px;
    border-radius: 8px;
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
  }

  .panel {
    display: grid;
    gap: 8px;
    animation: rise 0.45s 0.12s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .box {
    display: grid;
    gap: 6px;
    padding: 10px 10px 4px;
    border-radius: 16px;
    background: rgba(28, 28, 30, 0.78);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }
  .acts {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 4px;
  }
  .acts.four {
    grid-template-columns: repeat(4, 1fr);
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
    line-height: 1.2;
    text-align: center;
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
    gap: 2px;
    padding: 8px 0 8px;
    border-top: 1px solid var(--line);
  }
  .rate small {
    color: var(--muted);
    font-size: 11px;
  }
  .rate b {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 20px;
    font-variant-numeric: tabular-nums;
    color: var(--imdb);
  }
  /* padding — явно: класс .block у разделов листа совпадает с .btn.block. */
  .panel .btn {
    height: 36px;
    padding: 0 16px;
    border-radius: 999px;
    background: rgba(40, 40, 42, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.1);
    font-size: 13px;
    transition:
      background 0.2s,
      transform 0.2s;
  }
  .panel .btn:hover:not(:disabled) {
    background: rgba(70, 70, 74, 0.95);
    transform: translateY(-1px);
  }
  .lists {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .li {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 12px;
    border: 0;
    border-radius: 999px;
    background: rgba(40, 40, 42, 0.9);
    cursor: pointer;
    font-size: 13px;
    animation: rise 0.25s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .tick {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 5px;
    border: 1.5px solid var(--line-strong);
    color: transparent;
  }
  .li.on .tick {
    background: #fff;
    border-color: #fff;
    color: #000;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(14px);
    }
  }

  /* ---------- лист с разделами ---------- */
  .sheet {
    width: min(1440px, 100% - 2 * var(--gutter));
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
  .sub {
    margin: 22px 0 12px;
    font-size: 14px;
    color: var(--muted);
  }
  .lhead {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .vlinks {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 240px), 1fr));
    gap: 18px 16px;
  }
  .found {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .found button {
    display: grid;
    gap: 6px;
    width: 100%;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .found .art {
    position: relative;
    display: block;
    aspect-ratio: 16 / 9;
    border-radius: 10px;
    overflow: hidden;
    background: var(--surface);
    transition: transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .found button:hover .art {
    transform: translateY(-3px);
  }
  .found img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .found .p {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.92);
    color: #000;
  }
  .found .site,
  .found .dur {
    position: absolute;
    bottom: 6px;
    padding: 1px 6px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.72);
    font-size: 11px;
    font-weight: 700;
  }
  .found .site {
    left: 6px;
  }
  .found .dur {
    right: 6px;
  }
  .found .name {
    font-size: 13.5px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .none {
    margin: 0;
    font-size: 14px;
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
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 12px;
  }
  .similar a {
    position: relative;
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
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .similar span {
    padding: 10px;
    font-size: 13px;
    font-weight: 700;
  }
  .similar em {
    position: absolute;
    top: 8px;
    left: 8px;
    padding: 2px 7px;
    border-radius: 6px;
    background: #fff;
    color: #000;
    font-style: normal;
    font-size: 11px;
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
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
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
  .sk-hero {
    height: 70vh;
  }

  @media (max-width: 980px) {
    .wrap {
      grid-template-columns: 180px minmax(0, 1fr);
    }
    .panel {
      grid-column: 1 / -1;
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
    .sheet {
      width: 100%;
      border-radius: 18px 18px 0 0;
      padding-bottom: 32px;
    }
    .cast {
      grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
    }
    .cast .ava {
      width: 60px;
      height: 60px;
    }
    .similar {
      grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
    }
    .gallery {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    /* Под узким постером (110 px) — кнопка поменьше, чтобы «Найти видео» влезало. */
    .watch {
      height: 38px;
      padding: 0 6px;
      gap: 5px;
      font-size: 12.5px;
    }
  }
  :global(.no-motion) :is(.bg img, .bg .ph, .poster-col, .info, .panel, .li) {
    animation: none;
  }
</style>
