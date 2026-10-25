<script lang="ts">
  import { replaceState } from '$app/navigation';
  import { page } from '$app/state';
  import { api } from '$lib/api';
  import Poster from '$lib/discover/Poster.svelte';
  import type { Card, ExtCard } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import Wide from '$lib/ui/Wide.svelte';
  import { untrack } from 'svelte';
  import { catalog, loadCatalog } from '$lib/catalog.svelte';
  import { matches } from '$lib/library';
  import { isAnime } from '$lib/library';
  import { genreRu } from '$lib/genres';
  import Loading from '$lib/ui/Loading.svelte';
  import { personHref } from '$lib/people';

  // Поиск: своё (сразу) и IMDb - всё остальное (страница фильма - /ext). Поле - своё состояние:
  // раньше оно бралось из адреса и при быстром наборе теряло буквы (адрес отставал от ввода).
  let text = $state(untrack(() => page.url.searchParams.get('q') ?? ''));
  let items = $state<Card[] | null>(null);
  let found = $state<ExtCard[] | null>(null);
  let searching = $state(false);
  const query = $derived(text.trim());
  let kind = $state<'all' | 'movie' | 'series' | 'anime' | 'people'>('all');
  const scope = $derived(kind === 'people' ? 'people' : 'titles');
  let persons = $state<{id:string; name:string; description?:string; role:string}[]>([]);
  let year = $state('');
  let rating = $state('');
  let genre = $state('');
  let sort = $state('relevance');
  let failure = $state('');
  const cache = new Map<string, ExtCard[]>();
  const genres = $derived([...new Set([...(items ?? []).flatMap((c) => c.genres), ...(found ?? []).flatMap((c) => c.genres)].map(genreRu))].sort((a,b) => a.localeCompare(b,'ru')));
  function refine<T extends Card | ExtCard>(cards: T[]): T[] {
    const rows = cards.filter((c) => (kind === 'all' || (kind === 'anime' ? isAnime(c) : c.kind === kind)) && (!year || c.year === Number(year)) && (!rating || c.rating != null && c.rating >= Number(rating)) && (!genre || c.genres.some((g) => genreRu(g) === genre)));
    if (sort !== 'relevance') rows.sort((a,b) => sort === 'title' ? a.title.localeCompare(b.title,'ru') : sort === 'year' ? (b.year ?? 0)-(a.year ?? 0) : (b.rating ?? 0)-(a.rating ?? 0));
    return rows;
  }
  const localResults = $derived(refine(items ?? []));
  const externalResults = $derived(refine(found ?? []));
  $effect(() => { loadCatalog(); });

  $effect(() => {
    const q = query;
    const target = scope;
    persons = [];
    items = catalog.items?.filter((c) => matches(c, q)) ?? null;
    found = null;
    searching = false;
    failure = '';
    if (!q) return;
    const hit = cache.get(q.toLocaleLowerCase('ru'));
    if (hit && target !== 'people') { found = hit; return; }
    let live = true;
    const controller = new AbortController();
    let updateTimer = 0;
    const t = setTimeout(() => {
      try {
        replaceState(`/search?q=${encodeURIComponent(q)}`, {});
      } catch {
        // роутер ещё не готов - адрес не важен
      }
      if (!catalog.items) api<Card[]>(`/api/titles?sort=rating&q=${encodeURIComponent(q)}`, { signal: controller.signal }).then(
        (r) => live && (items = r),
        () => live && (items = []),
      );
      if (q.length < 2) return void (found = []);
      searching = true;
      if (target === 'people') {
        api<typeof persons>(`/api/discover/search?q=${encodeURIComponent(q)}&scope=people`, { signal: controller.signal }).then((r) => { if (live) persons = r; }, () => { if (live) failure = 'Не удалось загрузить людей кино'; }).finally(() => { if (live) { searching = false; found = []; } });
        return;
      }
      let refreshing=false;
      const load=()=>api<ExtCard[]>('/api/discover/search?q='+encodeURIComponent(q),{signal:controller.signal,headersReceived:(h)=>refreshing=h.get('x-nova-refreshing')==='1'})
        .then((r)=>{if(!live)return;const incoming=r.filter((e)=>!e.local);const old=found??[];const map=new Map(incoming.map((e)=>[e.id,e]));const seen=new Set(old.map((e)=>e.id));found=[...old.map((e)=>map.get(e.id)??e),...incoming.filter((e)=>!seen.has(e.id))];if(cache.size>=24)cache.delete(cache.keys().next().value!);if(!refreshing)cache.set(q.toLocaleLowerCase('ru'),found);if(refreshing)updateTimer=window.setTimeout(load,2500);},()=>{if(live){found=found??[];failure='Не удалось связаться с каталогом. Попробуйте ещё раз.';}})
        .finally(()=>{if(live)searching=false;});
      void load();
    }, 250);
    return () => {
      live = false;
      controller.abort();
      clearTimeout(updateTimer);
      clearTimeout(t);
    };
  });
</script>

<div class="page search-page">
  <header class="search-head"><span class="eyebrow">NOVA · ПОИСК</span><h1>Что посмотрим?</h1><p class="muted">Ваша коллекция и мир кино в одном поиске.</p></header>
  <form class="big" role="search" onsubmit={(e) => e.preventDefault()}>
    <Icon name="search" size={22} />
    <!-- svelte-ignore a11y_autofocus -->
    <input type="search" placeholder="Название фильма или сериала" bind:value={text} autofocus aria-label="Поиск" />
  </form>
  <nav class="filters" aria-label="Тип результатов">
    {#each [{ id: 'all', label: 'Всё' }, { id: 'movie', label: 'Фильмы' }, { id: 'series', label: 'Сериалы' }, { id: 'anime', label: 'Аниме' }, { id: 'people', label: 'Люди кино' }] as option (option.id)}
      <button class="chip" class:on={kind === option.id} onclick={() => kind = option.id as typeof kind}><Icon name={option.id === 'all' ? 'grid' : option.id === 'movie' ? 'film' : option.id === 'series' ? 'tv' : option.id === 'people' ? 'users' : 'star'} size={14} />{option.label}</button>
    {/each}
  </nav>
  <div class="refine" aria-label="Фильтры поиска">
    <label>Год<input type="text" inputmode="numeric" maxlength="4" placeholder="Любой" bind:value={year} /></label>
    <label>Рейтинг<select aria-label="Рейтинг" bind:value={rating}><option value="">Любой</option>{#each [5,6,7,8,9] as n}<option value={String(n)}>{n} и выше</option>{/each}</select></label>
    <label>Жанр<select aria-label="Жанр" bind:value={genre}><option value="">Все жанры</option>{#each genres as g}<option value={g}>{g}</option>{/each}</select></label>
    <label>Порядок<select aria-label="Порядок" bind:value={sort}><option value="relevance">По совпадению</option><option value="rating">По рейтингу</option><option value="year">Сначала новые</option><option value="title">По названию</option></select></label>
  </div>
  {#if failure}<p role="status">{failure}</p>{/if}
  {#if searching}<Loading compact label="Ищем в русских и международных каталогах" />{/if}
  {#if !query}
    <div class="search-empty"><Icon name="search" size={40} /><h2>Истории начинаются здесь</h2><p>Введите название, имя режиссёра или год.</p></div>
  {/if}
  {#if query}
    {#if kind === 'people'}
      <div class="persons">{#each persons as p (p.id)}<a href={personHref({name:p.name,job:p.role,id:p.id})}><Icon name="user" size={28} /><div><b>{p.name}</b><p class="muted">{p.description ?? 'Фильмография и биография'}</p></div><Icon name="right" size={16} /></a>{/each}</div>
      {#if !searching && !failure && !persons.length}<div class="empty">Никого не нашли. Попробуйте полное имя.</div>{/if}
    {:else}
    {#if localResults.length}
      <section aria-label="На Nova">
        <h2>На Nova</h2>
        <div class="grid">
          {#each localResults as card, i (card.id)}<Wide {card} {i} />{/each}
        </div>
      </section>
    {/if}
    {#if externalResults.length}
      <section aria-label="Все фильмы и сериалы">
        <h2>Все фильмы и сериалы</h2>
        <div class="posters">{#each externalResults as card, i (card.id)}<Poster {card} {i} />{/each}</div>
      </section>
    {:else if items === null || found === null || searching}
      <div class="posters">{#each Array(6) as _, i (i)}<span class="ph skeleton"></span>{/each}</div>
    {:else if !localResults.length}
      <div class="empty"><Icon name="search" size={40} /> По запросу «{query}» ничего нет</div>
    {/if}
    {/if}
  {/if}
</div>

<style>
  .search-page { max-width: 1680px; }
  .persons { display: grid; grid-template-columns: repeat(auto-fit,minmax(min(100%,300px),1fr)); gap: 16px; }
  .persons a { display: flex; gap: 16px; align-items: center; border: 1px solid var(--line); border-radius: 14px; padding: 20px; }
  .persons a div { min-width: 0; flex: 1; }.persons p { margin: 8px 0 0; font-size: 12px; }
  .search-head { margin: 20px 0 28px; }
  .eyebrow { color: var(--brand); font-size: 11px; letter-spacing: .2em; }
  h1 { font-size: clamp(32px, 4vw, 56px); margin-top: 12px; }
  .filters { display: flex; gap: 8px; margin-bottom: 28px; }
  .filters { flex-wrap: wrap; }
  .filters button { gap: 7px; }
  .refine { display: flex; flex-wrap: wrap; gap: 12px; margin-bottom: 28px; }
  .refine label { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border: 1px solid var(--line); border-radius: 12px; color: var(--muted); font-size: 12px; min-width: 0; }
  .refine input, .refine select { background: #000; color: #fff; border: 0; padding: 5px; font: inherit; max-width: 160px; }
  .refine input { width: 65px; }
  @media(max-width:600px) { .refine { gap: 8px; } .refine label { flex: 1 1 40%; } .refine select { flex: 1; min-width: 0; max-width: 100px; } }
  @media(max-width:380px) { .refine label { flex-basis: 100%; justify-content: space-between; } .refine select { max-width: 160px; } }
  .search-empty { display: grid; justify-items: center; gap: 14px; padding: 70px 20px; border: 1px dashed var(--line); border-radius: 24px; color: var(--muted); text-align: center; }
  .search-empty h2 { color: var(--text); margin: 0; }
  .search-empty p { margin: 0; }
  .big {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 56px;
    padding: 0 18px;
    margin-bottom: 28px;
    border-radius: 16px;
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .big:focus-within {
    border-color: var(--accent-2);
  }
  .big input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: none;
    font-size: 18px;
    color: var(--text);
  }
  h2 {
    margin: 0 0 14px;
    font-size: 20px;
  }
  section + section {
    margin-top: 32px;
  }
  .posters {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 20px 14px;
  }
  .ph {
    aspect-ratio: 2 / 3;
    border-radius: 12px;
  }
  @media (max-width: 600px) {
    .posters {
      grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
      gap: 16px 10px;
    }
  }
</style>
