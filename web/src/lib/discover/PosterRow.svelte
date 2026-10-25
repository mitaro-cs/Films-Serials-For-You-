<script lang="ts">
  import type { Card, ExtCard, Who } from '$lib/types';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Poster from './Poster.svelte';

  // Ряд постеров ленты - как ряд главной, только карточки вертикальные. «Все» - подборка целиком.
  // В «Друзья смотрят» под постером - кто и где остановился.
  type Tile = { ext?: ExtCard; card?: Card; person?: Who; label?: string };
  let {
    title,
    items,
    href = null,
    ranked = false,
    i = 0,
  }: { title: string; items: Tile[] | ExtCard[]; href?: string | null; ranked?: boolean; i?: number } = $props();

  // Элемент - карточка ленты или {ext | card} из «Буду смотреть» и списков.
  const tiles = $derived<Tile[]>(items.map((i) => ('id' in i ? { ext: i as ExtCard } : (i as Tile))));
  const key = (t: Tile, n: number) => `${t.person?.id ?? ''}:${t.ext ? t.ext.id : `t${t.card?.id}`}:${t.person ? n : ''}`;

  let strip = $state<HTMLDivElement>();
  let start = $state(true);
  let end = $state(true);

  function edges() {
    if (!strip) return;
    start = strip.scrollLeft < 8;
    end = strip.scrollLeft + strip.clientWidth > strip.scrollWidth - 8;
  }
  $effect(() => {
    void items.length;
    edges();
  });

  function page(dir: number) {
    strip?.scrollBy({ left: dir * strip.clientWidth * 0.85, behavior: 'smooth' });
  }
</script>

<section class="rise row" aria-label={title} style:--i={i}>
  <div class="head">
    <h2>{title}</h2>
    {#if href}<a class="all" {href}>Все <Icon name="right" size={16} /></a>{/if}
  </div>
  <div class="wrap">
    <div class="strip" bind:this={strip} onscroll={edges}>
      {#each tiles as t, n (key(t, n))}
        {#if t.person}
          <div class="by">
            <Poster card={t.ext} local={t.card ?? null} />
            <a class="who" href="/people/{t.person.id}"><Avatar avatar={t.person.avatar} photo={t.person.photo} size={20} /><span>{t.person.name}</span></a>
            {#if t.label}<small class="lbl">{t.label}</small>{/if}
          </div>
        {:else}
          <Poster card={t.ext} local={t.card ?? null} rank={ranked ? n + 1 : null} />
        {/if}
      {/each}
    </div>
    {#if !start}<button class="arrow l" aria-label="Назад" onclick={() => page(-1)}><Icon name="back" size={22} /></button>{/if}
    {#if !end}<button class="arrow r" aria-label="Дальше" onclick={() => page(1)}><Icon name="right" size={22} /></button>{/if}
  </div>
</section>

<style>
  .row {
    margin-bottom: 30px;
    --n: 7.4;
    --gap: 16px;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }
  h2 {
    font-size: clamp(19px, 1.6vw, 24px);
    font-weight: 700;
  }
  .all {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    color: var(--muted);
    font-size: 14px;
    font-weight: 600;
  }
  .all:hover {
    color: var(--text);
  }
  .wrap {
    position: relative;
  }
  .strip {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: calc((100% - (var(--n) - 1) * var(--gap)) / var(--n));
    gap: var(--gap);
    overflow-x: auto;
    margin: -12px calc(var(--gutter) * -1);
    padding: 12px var(--gutter);
    scroll-snap-type: x mandatory;
    scroll-padding: 0 var(--gutter);
    overscroll-behavior-x: contain;
  }
  .strip > :global(*) {
    scroll-snap-align: start;
  }
  .arrow {
    position: absolute;
    top: 0;
    bottom: 52px;
    width: calc(var(--gutter) - 4px);
    min-width: 34px;
    display: grid;
    place-items: center;
    border: 0;
    color: #fff;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.2s;
  }
  .wrap:hover .arrow,
  .arrow:focus-visible {
    opacity: 1;
  }
  .arrow.l {
    left: calc(var(--gutter) * -1);
    background: linear-gradient(90deg, rgba(0, 0, 0, 0.75), transparent);
  }
  .arrow.r {
    right: calc(var(--gutter) * -1);
    background: linear-gradient(270deg, rgba(0, 0, 0, 0.75), transparent);
  }
  @media (max-width: 1280px) {
    .row {
      --n: 6.3;
    }
  }
  @media (max-width: 1000px) {
    .row {
      --n: 4.4;
      --gap: 12px;
    }
  }
  /* Телефон - как у Disney+: три с лишним постера в ряд, плотно, без подписей (название - на постере
     и в описании для чтения с экрана). */
  @media (max-width: 640px) {
    .row {
      --n: 3.3;
      --gap: 8px;
      margin-bottom: 24px;
    }
    h2 {
      font-size: 17px;
    }
    .strip > :global(.tile .name),
    .strip > :global(.tile .sub) {
      display: none;
    }
  }
  @media (hover: none) {
    .arrow {
      display: none;
    }
  }
  .by {
    display: grid;
    align-content: start;
    gap: 4px;
    min-width: 0;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: 13px;
    font-weight: 600;
  }
  .who span,
  .lbl {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lbl {
    font-size: 12px;
    color: var(--muted);
  }
</style>
