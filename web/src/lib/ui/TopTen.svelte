<script lang="ts">
  import { hoverEnter, hoverLeave } from '$lib/cards.svelte';
  import type { Card } from '$lib/types';
  import { placeholder } from './art';

  // Ряд «Топ-10» в духе Netflix: огромная контурная цифра и постер рядом.
  let { card, n }: { card: Card; n: number } = $props();
  let el: HTMLAnchorElement | undefined = $state();
</script>

<a
  class="top"
  href="/title/{card.id}"
  bind:this={el}
  aria-label="{n} место: {card.title}"
  onmouseenter={() => el && hoverEnter(card, el)}
  onmouseleave={hoverLeave}
>
  <svg class="num" viewBox="0 0 110 160" aria-hidden="true">
    <text x={n === 10 ? 104 : 108} y="152" text-anchor="end">{n}</text>
  </svg>
  <span class="img" style:background={card.poster ? undefined : placeholder(card.title)}>
    {#if card.poster}<img src={card.poster} alt="" loading="lazy" decoding="async" />{:else}<b>{card.title}</b>{/if}
  </span>
</a>

<style>
  .top {
    position: relative;
    display: flex;
    align-items: flex-end;
    height: 100%;
  }
  .num {
    flex: none;
    width: 54%;
    margin-right: -14%;
    overflow: visible;
  }
  text {
    font-size: 170px;
    font-weight: 900;
    letter-spacing: -12px;
    fill: var(--bg);
    stroke: #8b86ad;
    stroke-width: 4px;
    paint-order: stroke;
    font-family: var(--font);
  }
  .img {
    position: relative;
    flex: none;
    width: 60%;
    aspect-ratio: 2 / 3;
    border-radius: 8px;
    overflow: hidden;
    box-shadow: -10px 0 24px -6px rgba(0, 0, 0, 0.7);
    transition: transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .top:hover .img {
    transform: translateY(-4px) scale(1.03);
  }
  .img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .img b {
    position: absolute;
    inset: auto 8px 10px;
    font-size: 14px;
  }
</style>
