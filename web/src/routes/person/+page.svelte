<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import { page } from '$app/state';
  import { api, errorText } from '$lib/api';
  import { count } from '$lib/fmt';
  import Poster from '$lib/discover/Poster.svelte';
  import type { Card, ExtCard } from '$lib/types';
  import { artW, placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';

  // Человек кино (режиссёр или актёр): круглое фото, имя, фильмография с IMDb (по годам, свежее
  // первым) и что из этого есть на Nova. Первый раз сервер берёт её с IMDb - несколько секунд.
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

  $effect(() => {
    const want = name;
    void retry;
    const controller = new AbortController();
    const q = new URLSearchParams({ name: want, role });
    if (pid) q.set('id', pid);
    d = null;
    error = '';
    kind = 'all';
    if (!want) return void (error = 'Не указано, кто это');
    api<Director>(`/api/people/imdb?${q}`, { signal: controller.signal }).then(
      (r) => !controller.signal.aborted && (d = r),
      (e) => { if (!controller.signal.aborted) error = errorText(e); },
    );
    return () => controller.abort();
  });

  const works = $derived(d ? d.works.filter((w) => kind === 'all' || w.kind === kind) : []);
  const years = $derived.by(() => {
    const ys = (d?.works ?? []).map((w) => w.year).filter((y): y is number => !!y);
    return ys.length ? `${Math.min(...ys)} - ${Math.max(...ys)}` : '';
  });
  const highlights = $derived.by(() => {
    const works = d?.works ?? [];
    const genres = new Map<string, number>();
    for (const w of works) for (const g of w.genres) genres.set(g, (genres.get(g) ?? 0) + 1);
    return {
      genres: [...genres].sort((a, b) => b[1] - a[1]).slice(0, 3).map(([g]) => g).join(', '),
      best: [...works].filter((w) => w.rating).sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0)).slice(0, 3),
    };
  });
  const initials = (s: string) =>
    s
      .split(/\s+/)
      .slice(0, 2)
      .map((w) => w[0])
      .join('');
</script>

<div class="page">
  {#if error}
    <div class="empty"><Icon name="info" size={36} /> {error}</div>
  {:else}
    <header class="head">
      <span class="ph" style:background={d?.photo ? undefined : placeholder(name)}>
        {#if d?.photo}<img use:fadeIn src={artW(d.photo, 640)} alt={name} width="400" height="400" decoding="async" />{:else if d}{initials(name)}{/if}
      </span>
      <div class="who">
        <small class="muted">{role === 'actor' ? 'Актёр' : 'Режиссёр'}</small>
        <h1>{d?.info?.nameRu ?? name}</h1>
        {#if d?.imdbName && d.imdbName !== name}<p class="muted orig">{d.imdbName}</p>{/if}
        {#if d}
          <p class="facts">
            {#if d.works.length}<span><Icon name="film" size={15} /> {count(d.works.length, 'работа', 'работы', 'работ')}</span>{/if}
            {#if years}<span><Icon name="clock" size={15} /> {years}</span>{/if}
            {#if d.local.length}<span class="here"><Icon name="play" size={14} /> {d.local.length} на Nova</span>{/if}
            {#if d.id}<a class="imdb-l" href="https://www.imdb.com/name/{d.id}/" target="_blank" rel="noopener noreferrer"><b class="imdb">IMDb</b></a>{/if}
          </p>
        {:else}
          <p class="muted">Собираем фильмографию с IMDb…</p>
        {/if}
      </div>
    </header>

    {#if d === null}
      <div class="grid">{#each Array(12) as _, i (i)}<span class="sk skeleton"></span>{/each}</div>
    {:else}
      <section class="person-summary card" aria-label="Кратко о человеке">
        <h2>Биография</h2>
        {#if d.info?.birthDate}<p class="muted">Дата рождения: {new Date(d.info.birthDate).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' })}</p>{/if}
        {#if d.info?.biography || d.info?.description}<p>{d.info.biography ?? d.info.description}</p>{:else}<p class="muted">Не удалось загрузить биографию на русском. Попробуйте обновить сведения или откройте страницу источника.</p><button class="btn sm" onclick={() => retry++}>Обновить сведения</button>{/if}
        {#if d.info?.source}<p class="sources">Источники: <a href={d.info.source} target="_blank" rel="noopener noreferrer">Викиданные</a>{#if d.info.wiki} · <a href={d.info.wiki} target="_blank" rel="noopener noreferrer">Википедия</a>{/if}</p>{/if}
        {#if highlights.best.length}<p class="muted">Среди работ с высокой оценкой IMDb: {#each highlights.best as w, i (w.id)}{i ? ', ' : ''}<a href="/ext/{w.id}">{w.title}{w.year ? ` (${w.year})` : ''}</a>{/each}.</p>{/if}
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
  .person-summary { padding: 24px; margin-bottom: 28px; }
  .person-summary h2 { font-size: 20px; }
  .person-summary p { max-width: 95ch; margin-bottom: 0; }
  .person-summary a { color: var(--brand); text-decoration: underline; text-underline-offset: 3px; }
  .person-summary .sources { font-size: 12px; margin-top: 16px; color: var(--muted); }
  .head {
    display: flex;
    align-items: center;
    gap: 28px;
    margin: 12px 0 36px;
  }
  .ph {
    flex: none;
    display: grid;
    place-items: center;
    width: clamp(110px, 14vw, 170px);
    aspect-ratio: 1;
    border-radius: 50%;
    overflow: hidden;
    font-size: 40px;
    font-weight: 800;
    box-shadow:
      0 0 0 4px #000,
      0 0 0 6px rgba(255, 255, 255, 0.18),
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
      transform: scale(0.8);
    }
  }
  .who {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .who small {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  h1 {
    font-size: clamp(30px, 4.6vw, 56px);
    font-weight: 800;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }
  .orig {
    margin: 0;
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 6px 0 0;
  }
  .facts span,
  .imdb-l {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    font-size: 13px;
    font-weight: 600;
  }
  .facts .here {
    background: #fff;
    color: #000;
  }
  section {
    margin-bottom: 34px;
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
  @media (max-width: 640px) {
    .head {
      flex-direction: column;
      text-align: center;
      gap: 16px;
    }
    .facts {
      justify-content: center;
    }
    .grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 16px 10px;
    }
  }
</style>
