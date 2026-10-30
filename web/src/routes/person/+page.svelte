<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import { page } from '$app/state';
  import { api, errorText, patch, put } from '$lib/api';
  import { count } from '$lib/fmt';
  import Poster from '$lib/discover/Poster.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import { toneOfUrl } from '$lib/ui/tone';
  import type { Card, ExtCard } from '$lib/types';
  import { artW, placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';

  // Человек кино (режиссёр или актёр): шапка с мозаикой лучших постеров в цвет, «нравится» и любимый
  // режиссёр, лучшие фильмы по оценке IMDb, биография, фильмография (свежее первым) и что есть на Nova.
  // Первый раз сервер берёт её с IMDb - несколько секунд.
  type Director = {
    name: string;
    imdbName: string | null;
    id: string | null;
    role: 'director' | 'actor';
    photo: string | null;
    works: ExtCard[];
    local: Card[];
    availability?: 'full' | 'partial' | 'unavailable';
    info?: { nameRu?: string | null; biography?: string | null; description?: string | null; birthDate?: string | null; source?: string; wiki?: string | null } | null;
  };

  const name = $derived(page.url.searchParams.get('name') ?? '');
  const pid = $derived(page.url.searchParams.get('id') ?? '');
  const role = $derived(page.url.searchParams.get('role') === 'actor' ? 'actor' : 'director');
  let d = $state<Director | null>(null);
  let error = $state('');
  let kind = $state<'all' | 'movie' | 'series'>('all');
  let retry = $state(0);
  let tone = $state('#1a1b20');
  let more = $state(false);

  $effect(() => {
    const want = name;
    void retry;
    const controller = new AbortController();
    const q = new URLSearchParams({ name: want, role });
    if (pid) q.set('id', pid);
    d = null;
    error = '';
    kind = 'all';
    more = false;
    if (!want) return void (error = 'Не указано, кто это');
    api<Director>(`/api/people/imdb?${q}`, { signal: controller.signal }).then(
      (r) => !controller.signal.aborted && (d = r),
      (e) => { if (!controller.signal.aborted) error = errorText(e); },
    );
    return () => controller.abort();
  });

  // «Нравится» и любимый режиссёр (один на профиль).
  let liked = $state<{ name: string; role: string }[]>([]);
  let favDirectors = $state<string[]>([]);
  $effect(() => {
    api<{ liked: { name: string; role: string }[]; directors: string[] }>('/api/me/people').then(
      (r) => { liked = r.liked; favDirectors = r.directors; },
      () => {},
    );
  });
  const isLiked = $derived(liked.some((p) => p.name === name && p.role === role));
  const isFavDirector = $derived(favDirectors.some((n) => n.toLowerCase() === name.toLowerCase()));
  async function like() {
    const value = !isLiked;
    try {
      const r = await put<{ liked: { name: string; role: string }[] }>('/api/me/people', { name, role, value });
      liked = r.liked;
    } catch (e) {
      toast(errorText(e));
    }
  }
  async function favDirector() {
    try {
      const next = isFavDirector ? [] : [name];
      await patch('/api/me/profile', { directors: next });
      favDirectors = next;
      toast(next.length ? 'Теперь это ваш любимый режиссёр' : 'Любимый режиссёр убран');
    } catch (e) {
      toast(errorText(e));
    }
  }

  const works = $derived(d ? d.works.filter((w) => kind === 'all' || w.kind === kind) : []);
  const years = $derived.by(() => {
    const ys = (d?.works ?? []).map((w) => w.year).filter((y): y is number => !!y);
    return ys.length ? `${Math.min(...ys)} - ${Math.max(...ys)}` : '';
  });
  // Лучшие фильмы: по оценке IMDb, у кого уже набралось оценок, сначала фильмы.
  const best = $derived(
    (d?.works ?? [])
      .filter((w) => w.rating && w.poster)
      .sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0))
      .slice(0, 10),
  );
  const mosaic = $derived((d?.works ?? []).filter((w) => w.poster).slice(0, 18));
  $effect(() => {
    const src = best[0]?.poster;
    if (!src) return;
    let live = true;
    void toneOfUrl(artW(src, 342) ?? src).then((c) => { if (live && c) tone = c; });
    return () => { live = false; };
  });
  const initials = (s: string) =>
    s
      .split(/\s+/)
      .slice(0, 2)
      .map((w) => w[0])
      .join('');
  const bio = $derived(d?.info?.biography ?? d?.info?.description ?? '');
</script>

<div class="page" style:--tone={tone}>
  {#if error}
    <div class="empty"><Icon name="info" size={36} /> {error}</div>
  {:else}
    <header class="hero">
      <div class="wall" aria-hidden="true">
        {#each mosaic as w (w.id)}<img src={artW(w.poster, 342)} alt="" loading="lazy" decoding="async" />{/each}
      </div>
      <div class="veil" aria-hidden="true"></div>
      <div class="head">
        <span class="ph" style:background={d?.photo ? undefined : placeholder(name)}>
          {#if d?.photo}<img use:fadeIn src={artW(d.photo, 640)} alt={name} width="400" height="400" decoding="async" />{:else if d}{initials(name)}{/if}
        </span>
        <div class="who">
          <small>{role === 'actor' ? 'Актёр' : 'Режиссёр'}</small>
          <h1>{d?.info?.nameRu ?? name}</h1>
          {#if d?.imdbName && d.imdbName !== name}<p class="orig">{d.imdbName}</p>{/if}
          {#if d}
            <p class="facts">
              {#if d.works.length}<span><Icon name="film" size={15} /> {count(d.works.length, 'работа', 'работы', 'работ')}</span>{/if}
              {#if years}<span><Icon name="clock" size={15} /> {years}</span>{/if}
              {#if d.local.length}<span class="here"><Icon name="play" size={14} /> {d.local.length} на Nova</span>{/if}
              {#if d.id}<a class="imdb-l" href="https://www.imdb.com/name/{d.id}/" target="_blank" rel="noopener noreferrer"><b class="imdb">IMDb</b></a>{/if}
            </p>
            <div class="acts">
              <button class="act" class:on={isLiked} aria-pressed={isLiked} onclick={like}>
                <Icon name="heart" size={18} /> {isLiked ? 'Вам нравится' : 'Нравится'}
              </button>
              {#if role === 'director'}
                <button class="act" class:on={isFavDirector} aria-pressed={isFavDirector} onclick={favDirector}>
                  <Icon name="star" size={18} /> {isFavDirector ? 'Любимый режиссёр' : 'Сделать любимым режиссёром'}
                </button>
              {/if}
            </div>
          {:else}
            <p class="orig">Собираем фильмографию с IMDb…</p>
          {/if}
        </div>
      </div>
    </header>

    {#if d === null}
      <div class="grid">{#each Array(12) as _, i (i)}<span class="sk skeleton"></span>{/each}</div>
    {:else}
      {#if best.length}
        <section aria-labelledby="h-best">
          <h2 id="h-best">Лучшие фильмы</h2>
          <ol class="best">
            {#each best as w, i (w.id)}
              <li style:--i={i}>
                <span class="rank" aria-hidden="true">{i + 1}</span>
                <Poster card={w} {i} />
              </li>
            {/each}
          </ol>
        </section>
      {/if}

      <section class="person-summary" aria-label="Кратко о человеке">
        <h2>Биография</h2>
        {#if d.info?.birthDate}<p class="muted">Дата рождения: {new Date(d.info.birthDate).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' })}</p>{/if}
        {#if bio}
          <p class="text" class:clamp={!more && bio.length > 420}>{bio}</p>
          {#if bio.length > 420}<button class="link" onclick={() => (more = !more)}>{more ? 'Свернуть' : 'Читать дальше'}</button>{/if}
        {:else}<p class="muted">Не удалось загрузить биографию на русском. Попробуйте обновить сведения или откройте страницу источника.</p><button class="btn sm" onclick={() => retry++}>Обновить сведения</button>{/if}
        {#if d.info?.source}<p class="sources">Источники: <a href={d.info.source} target="_blank" rel="noopener noreferrer">Викиданные</a>{#if d.info.wiki} · <a href={d.info.wiki} target="_blank" rel="noopener noreferrer">Википедия</a>{/if}</p>{/if}
      </section>
      {#if d.local.length}
        <section aria-labelledby="h-local">
          <h2 id="h-local">Есть на Nova</h2>
          <div class="grid">{#each d.local as c, i (c.id)}<Poster local={c} {i} />{/each}</div>
        </section>
      {/if}
      <section aria-labelledby="h-works">
        {#if d.availability === 'partial'}<p class="muted">Показаны работы из Викиданных и сохранённые сведения. Полную фильмографию IMDb сейчас загрузить не удалось.</p>{/if}
        <div class="bar">
          <h2 id="h-works">Фильмография</h2>
          <div class="chips" role="tablist" aria-label="Что показывать">
            {#each [{ id: 'all', l: 'Всё' }, { id: 'movie', l: 'Фильмы' }, { id: 'series', l: 'Сериалы' }] as k (k.id)}
              <button class="chip" class:on={kind === k.id} role="tab" aria-selected={kind === k.id} onclick={() => (kind = k.id as typeof kind)}>{k.l}</button>
            {/each}
          </div>
        </div>
        {#if works.length}
          <div class="grid">{#each works as w, i (w.id)}<Poster card={w} {i} />{/each}</div>
        {:else}
          <p class="muted">{d.availability === 'unavailable' ? 'Сведения о работах сейчас недоступны. Это ошибка загрузки, а не отсутствие фильмографии.' : d.id ? 'Для этого фильтра работы не найдены.' : 'Не удалось определить человека. Попробуйте поиск по оригинальному имени.'}</p><button class="btn sm" onclick={() => retry++}>Повторить загрузку</button>
        {/if}
      </section>
    {/if}
  {/if}
</div>

<style>
  .hero {
    position: relative;
    isolation: isolate;
    margin: 0 calc(-1 * var(--gutter)) 34px;
    padding: calc(var(--header) + 28px) var(--gutter) 34px;
    overflow: hidden;
    background: color-mix(in srgb, var(--tone) 50%, #000);
    transition: background-color 0.8s ease;
  }
  /* Мозаика постеров лучших работ плывёт на фоне: справа ярче, слева растворяется в цвет. */
  .wall {
    position: absolute;
    inset: 0 0 0 28%;
    z-index: -2;
    display: grid;
    grid-template-columns: repeat(9, minmax(0, 1fr));
    grid-auto-rows: 1fr;
    gap: 8px;
    padding: 8px;
    opacity: 0.55;
    transform: rotate(-4deg) scale(1.25);
  }
  .wall img {
    width: 100%;
    aspect-ratio: 2 / 3;
    object-fit: cover;
    border-radius: 8px;
  }
  .veil {
    position: absolute;
    inset: 0;
    z-index: -1;
    background:
      linear-gradient(90deg, color-mix(in srgb, var(--tone) 50%, #000) 18%, transparent 75%),
      linear-gradient(0deg, var(--bg) 0%, transparent 55%);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 30px;
    max-width: 1440px;
    margin: 0 auto;
  }
  .ph {
    flex: none;
    display: grid;
    place-items: center;
    width: clamp(120px, 14vw, 210px);
    aspect-ratio: 1;
    border-radius: 28%;
    overflow: hidden;
    font-size: 44px;
    font-weight: 800;
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.18),
      0 30px 60px -20px rgba(0, 0, 0, 0.9);
    animation: pop 0.6s cubic-bezier(0.2, 0.9, 0.3, 1.3) both;
  }
  .ph img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.85);
    }
  }
  .who {
    display: grid;
    gap: 8px;
    min-width: 0;
  }
  .who small {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #ffffffb3;
  }
  h1 {
    font-size: clamp(30px, 4.4vw, 60px);
    line-height: 1.05;
    font-weight: 800;
    letter-spacing: -0.02em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding-bottom: 0.08em;
  }
  .orig {
    margin: 0;
    color: #ffffffa6;
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 4px 0 0;
  }
  .facts span,
  .imdb-l {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.1);
    font-size: 13px;
    font-weight: 600;
  }
  .facts .here {
    background: #fff;
    color: #000;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 10px;
  }
  .act {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 42px;
    padding: 0 18px;
    border: 0;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.14);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.18);
    color: #fff;
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition:
      background 0.2s,
      transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.4);
  }
  .act:hover {
    background: rgba(255, 255, 255, 0.24);
    transform: translateY(-2px);
  }
  .act:active {
    transform: scale(0.96);
  }
  .act.on {
    background: #fff;
    color: #0a0a0a;
  }
  .act.on :global(svg) {
    fill: currentColor;
  }

  section {
    margin-bottom: 38px;
  }
  .best {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 26px 16px;
  }
  .best li {
    position: relative;
    animation: rise-in 0.5s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(var(--i) * 40ms);
  }
  .rank {
    position: absolute;
    z-index: 2;
    left: -6px;
    top: -10px;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: #fff;
    color: #0a0a0a;
    font-weight: 800;
    font-size: 15px;
    box-shadow: 0 6px 16px -6px #000;
    pointer-events: none;
  }
  .person-summary {
    padding: 28px 32px;
    border-radius: 26px;
    background: #1b1b1b;
  }
  .person-summary h2 {
    margin-bottom: 12px;
  }
  .person-summary p {
    max-width: 95ch;
    margin: 0 0 10px;
    line-height: 1.65;
  }
  .person-summary .text.clamp {
    display: -webkit-box;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .person-summary .link {
    padding: 0;
    border: 0;
    background: none;
    color: #fff;
    font: inherit;
    font-weight: 600;
    text-decoration: underline;
    cursor: pointer;
  }
  .person-summary a {
    color: var(--brand);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .person-summary .sources {
    font-size: 12px;
    margin-top: 16px;
    color: var(--muted);
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px 16px;
    margin-bottom: 16px;
  }
  h2 {
    font-size: clamp(19px, 1.6vw, 24px);
    font-weight: 700;
    margin-bottom: 16px;
  }
  .bar h2 {
    margin: 0;
  }
  .chips {
    display: flex;
    gap: 8px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 150px), 1fr));
    gap: 22px 16px;
  }
  .sk {
    aspect-ratio: 2 / 3;
    border-radius: 12px;
  }
  @media (max-width: 980px) {
    .best {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
  @media (max-width: 640px) {
    .head {
      flex-direction: column;
      text-align: center;
      gap: 16px;
    }
    .facts,
    .acts {
      justify-content: center;
    }
    .wall {
      inset: 0;
      grid-template-columns: repeat(5, minmax(0, 1fr));
    }
    h1 {
      white-space: normal;
    }
    .best,
    .grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 16px 10px;
    }
    .person-summary {
      padding: 20px;
    }
  }
  :global(.no-motion) :is(.ph, .best li) {
    animation: none;
  }
</style>
