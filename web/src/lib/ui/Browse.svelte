<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api } from '$lib/api';
  import { count } from '$lib/fmt';
  import type { Card, Kind } from '$lib/types';
  import Icon from './Icon.svelte';
  import Poster from './Poster.svelte';

  let { kind, title }: { kind: Kind | null; title: string } = $props();

  let items = $state<Card[] | null>(null);
  let genres = $state<{ name: string; count: number }[]>([]);
  const genre = $derived(page.url.searchParams.get('genre') ?? '');
  const sort = $derived(page.url.searchParams.get('sort') ?? 'added');

  $effect(() => {
    api<{ name: string; count: number }[]>('/api/genres').then((g) => (genres = g), () => {});
  });

  $effect(() => {
    const q = new URLSearchParams({ sort });
    if (kind && !page.url.searchParams.get('all')) q.set('kind', kind);
    if (genre) q.set('genre', genre);
    items = null;
    api<Card[]>(`/api/titles?${q}`).then((r) => (items = r), () => (items = []));
  });

  function set(key: string, value: string) {
    const u = new URL(page.url);
    if (value) u.searchParams.set(key, value);
    else u.searchParams.delete(key);
    goto(u, { replaceState: true, noScroll: true, keepFocus: true });
  }

  const SORTS = [
    ['added', 'Недавно добавленные'],
    ['rating', 'По рейтингу'],
    ['year', 'По году'],
    ['title', 'По названию'],
  ];
</script>

<div class="page">
  <div class="head">
    <h1>{genre || title}</h1>
    {#if items}<span class="muted">{count(items.length, 'название', 'названия', 'названий')}</span>{/if}
    <span class="grow"></span>
    <select class="input sort" value={sort} onchange={(e) => set('sort', (e.target as HTMLSelectElement).value)} aria-label="Сортировка">
      {#each SORTS as [v, l] (v)}<option value={v}>{l}</option>{/each}
    </select>
  </div>
  {#if genres.length}
    <div class="genres">
      <button class="chip" class:on={!genre} onclick={() => set('genre', '')}>Все жанры</button>
      {#each genres as g (g.name)}
        <button class="chip" class:on={genre === g.name} onclick={() => set('genre', g.name)}>{g.name}</button>
      {/each}
    </div>
  {/if}
  {#if items === null}
    <div class="grid">
      {#each Array(12) as _, i (i)}<div class="ph skeleton"></div>{/each}
    </div>
  {:else if items.length}
    <div class="grid">
      {#each items as card (card.id)}<Poster {card} />{/each}
    </div>
  {:else}
    <div class="empty"><Icon name="film" size={40} /> Ничего не нашлось</div>
  {/if}
</div>

<style>
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 16px;
    margin-bottom: 18px;
  }
  h1 {
    font-size: 30px;
  }
  .grow {
    flex: 1;
  }
  .sort {
    width: auto;
    height: 38px;
    border-radius: 999px;
  }
  .genres {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    margin: 0 calc(var(--gutter) * -1) 26px;
    padding: 0 var(--gutter);
  }
  .ph {
    aspect-ratio: 2 / 3;
    border-radius: 10px;
  }
</style>
