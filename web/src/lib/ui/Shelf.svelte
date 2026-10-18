<script lang="ts">
  import type { Card } from '$lib/types';
  import Icon from './Icon.svelte';
  import Wide from './Wide.svelte';

  // Ряд главной: заголовок и лента широких карточек; на компьютере — стрелки у краёв
  // (появляются при наведении, место не занимают).
  let { title, items, eager = false, i = 0 }: { title: string; items: Card[]; eager?: boolean; i?: number } = $props();

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

<section class="rise shelf" aria-label={title} style:--i={i}>
  <h2>{title}</h2>
  <div class="wrap">
    <div class="strip" bind:this={strip} onscroll={edges}>
      {#each items as card, i (card.id)}<Wide {card} eager={eager && i < 5} />{/each}
    </div>
    {#if !start}<button class="arrow l" aria-label="Назад" onclick={() => page(-1)}><Icon name="back" size={22} /></button>{/if}
    {#if !end}<button class="arrow r" aria-label="Дальше" onclick={() => page(1)}><Icon name="right" size={22} /></button>{/if}
  </div>
</section>

<style>
  .shelf {
    margin-bottom: 30px;
    --n: 5.3;
    --gap: 18px;
  }
  h2 {
    font-size: clamp(19px, 1.6vw, 24px);
    font-weight: 700;
    margin-bottom: 12px;
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
    .shelf {
      --n: 4.3;
    }
  }
  @media (max-width: 1000px) {
    .shelf {
      --n: 3.2;
      --gap: 14px;
    }
  }
  @media (max-width: 640px) {
    .shelf {
      --n: 1.75;
      --gap: 12px;
    }
  }
  @media (hover: none) {
    .arrow {
      display: none;
    }
  }
</style>
