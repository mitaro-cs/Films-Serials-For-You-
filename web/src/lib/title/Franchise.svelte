<script lang="ts">
  import { api } from '$lib/api';
  import Poster from '$lib/discover/Poster.svelte';
  import type { ExtCard } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';

  // Хронология франшизы («Гарри Поттер», «Стражи Галактики», вселенная Marvel): все фильмы серии
  // по порядку, этот - отмечен. Серий может быть несколько - от узкой к широкой.
  let { imdb }: { imdb: string } = $props();

  type Group = { name: string; items: ExtCard[] };
  let groups = $state<Group[]>([]);
  let loading = $state(true);
  let failed = $state(false);
  const strips: HTMLOListElement[] = $state([]);

  $effect(() => {
    const want = imdb;
    groups = [];
    loading = true;
    failed = false;
    api<Group[]>(`/api/franchise?imdb=${encodeURIComponent(want)}`).then(
      (g) => want === imdb && (groups = g),
      () => { if (want === imdb) failed = true; },
    ).finally(() => { if (want === imdb) loading = false; });
  });

  // Лента сразу на этом фильме (без прокрутки страницы).
  $effect(() => {
    void groups.length;
    for (const s of strips) {
      const el = s?.querySelector<HTMLElement>('.here');
      if (el) s.scrollLeft = el.offsetLeft - s.clientWidth / 2 + el.clientWidth / 2;
    }
  });
</script>

{#if loading}<div class="chronology-loading" aria-label="Загружаем порядок просмотра" aria-busy="true">{#each Array(6) as _, i (i)}<span class="skeleton"></span>{/each}</div>{/if}
{#if failed}<p class="muted">Порядок просмотра сейчас недоступен.</p>{/if}
{#each groups as g, n (g.name)}
  <section class="block" aria-label="Хронология: {g.name}">
    <div class="heading"><h2>Порядок просмотра <span class="muted">· {g.name}</span></h2><div class="arrows"><a class="btn sm" href="/franchise/{encodeURIComponent(imdb)}?group={n}">Открыть подборку</a><button class="btn sm" aria-label="Предыдущие фильмы" onclick={() => strips[n]?.scrollBy({ left: -strips[n].clientWidth * .8, behavior: 'smooth' })}><Icon name="back" size={16} /></button><button class="btn sm" aria-label="Следующие фильмы" onclick={() => strips[n]?.scrollBy({ left: strips[n].clientWidth * .8, behavior: 'smooth' })}><span class="forward"><Icon name="back" size={16} /></span></button></div></div>
    <p class="muted small">{g.items.length} частей · порядок серии и даты выхода по Викиданным</p>
    <ol class="strip" bind:this={strips[n]}>
      {#each g.items as card, i (card.id)}
        <li class:here={card.id === imdb}>
          <span class="no" aria-hidden="true">{i + 1}</span>
          <Poster {card} />
          {#if card.id === imdb}<span class="now">Этот фильм</span>{/if}
        </li>
      {/each}
    </ol>
  </section>
{/each}

<style>
  .heading { display: flex; justify-content: space-between; align-items: center; gap: 16px; margin-bottom: 16px; }
  .heading h2 { margin: 0; }
  .arrows { display: flex; gap: 6px; flex: none; }
  .forward { transform: rotate(180deg); display: flex; }
  .chronology-loading { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: 14px; padding: 24px 0; }
  .chronology-loading span { aspect-ratio: 2 / 3; border-radius: 14px; }
  @media(max-width: 600px) { .heading { align-items: start; flex-wrap: wrap; gap: 10px; } .heading h2 { font-size: 19px; min-width: 0; overflow-wrap: anywhere; } .arrows button { display: none; } }
  h2 .muted {
    font-weight: 600;
  }
  .small {
    margin: -6px 0 14px;
    font-size: 13px;
  }
  .strip {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 150px;
    gap: 14px;
    margin: 0;
    padding: 4px 2px 8px;
    list-style: none;
    overflow-x: auto;
    scrollbar-width: none;
    overscroll-behavior-x: contain;
  }
  .strip::-webkit-scrollbar {
    display: none;
  }
  li {
    position: relative;
    min-width: 0;
    display: grid;
    gap: 6px;
    align-content: start;
  }
  .no {
    position: absolute;
    z-index: 2;
    top: 8px;
    left: 8px;
    min-width: 26px;
    height: 26px;
    padding: 0 7px;
    border-radius: 13px;
    background: rgba(10, 10, 12, 0.82);
    color: #fff;
    font-size: 13px;
    font-weight: 700;
    line-height: 26px;
    text-align: center;
    pointer-events: none;
  }
  .here .no {
    background: #fff;
    color: #0a0a0a;
  }
  .here :global(.art) {
    outline: 2px solid #fff;
    outline-offset: 2px;
  }
  .now {
    justify-self: start;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--surface-2);
    font-size: 12px;
    font-weight: 600;
  }
  @media (max-width: 600px) {
    .strip {
      grid-auto-columns: 112px;
      gap: 10px;
    }
  }
</style>
