<script lang="ts">
  import type { Card } from '$lib/types';
  import { placeholder } from './art';
  import Stars from './Stars.svelte';

  let { card, width }: { card: Card; width?: number } = $props();
  let failed = $state(false);
</script>

<a class="poster" href="/title/{card.id}" style:width={width ? `${width}px` : undefined}>
  <div class="img" style:background={card.poster && !failed ? undefined : placeholder(card.title)}>
    {#if card.poster && !failed}
      <img src={card.poster} alt="" loading="lazy" decoding="async" onerror={() => (failed = true)} />
    {:else}
      <span class="ph">{card.title}</span>
    {/if}
    {#if card.kind === 'series'}<span class="tag">Сериал</span>{/if}
  </div>
  <div class="t" title={card.title}>{card.title}</div>
  <div class="sub">
    {#if card.originalTitle && card.originalTitle !== card.title}{card.originalTitle}{:else}{card.year ?? ''}{/if}
  </div>
  {#if card.rating}
    <div class="rate">
      <Stars rating={card.rating} />
      <span class="imdb">IMDb <b>{card.rating.toFixed(1)}</b></span>
    </div>
  {/if}
</a>

<style>
  .poster {
    display: block;
    min-width: 0;
  }
  .img {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: 10px;
    overflow: hidden;
    background: var(--surface);
    box-shadow: 0 10px 30px -14px rgba(0, 0, 0, 0.8);
    transition:
      transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1),
      box-shadow 0.25s;
  }
  .poster:hover .img {
    transform: translateY(-4px) scale(1.02);
    box-shadow: 0 18px 40px -16px rgba(138, 99, 255, 0.55);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .ph {
    position: absolute;
    inset: auto 12px 14px;
    font-weight: 800;
    font-size: 17px;
    line-height: 1.15;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.5);
  }
  .tag {
    position: absolute;
    top: 8px;
    left: 8px;
    padding: 2px 8px;
    border-radius: 6px;
    background: rgba(11, 10, 26, 0.75);
    backdrop-filter: blur(6px);
    font-size: 11px;
    font-weight: 700;
  }
  .t {
    margin-top: 10px;
    font-weight: 600;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-height: 18px;
  }
  .rate {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
  }
  .rate .imdb {
    height: 18px;
    font-size: 10px;
  }
</style>
