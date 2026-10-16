<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let { title, href, children }: { title: string; href?: string; children: Snippet } = $props();
  let track: HTMLDivElement | undefined = $state();
  let atStart = $state(true);
  let atEnd = $state(false);

  function update() {
    if (!track) return;
    atStart = track.scrollLeft < 8;
    atEnd = track.scrollLeft + track.clientWidth > track.scrollWidth - 8;
  }

  function scroll(dir: number) {
    track?.scrollBy({ left: dir * track.clientWidth * 0.85, behavior: 'smooth' });
  }

  $effect(() => {
    update();
  });
</script>

<section class="row">
  <div class="head">
    <h2>{title}</h2>
    {#if href}<a class="all" {href}>Все <Icon name="right" size={16} /></a>{/if}
  </div>
  <div class="wrap">
    {#if !atStart}
      <button class="arrow l" aria-label="Назад" onclick={() => scroll(-1)}><Icon name="back" size={26} /></button>
    {/if}
    <div class="track" bind:this={track} onscroll={update}>
      {@render children()}
    </div>
    {#if !atEnd}
      <button class="arrow r" aria-label="Дальше" onclick={() => scroll(1)}><Icon name="right" size={26} /></button>
    {/if}
  </div>
</section>

<style>
  .row {
    margin-top: 34px;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 0 var(--gutter);
    margin-bottom: 12px;
  }
  h2 {
    font-size: 19px;
  }
  .all {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 13px;
    color: var(--muted);
  }
  .all:hover {
    color: var(--text);
  }
  .wrap {
    position: relative;
  }
  .track {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: var(--col, 168px);
    gap: 16px;
    overflow-x: auto;
    scroll-snap-type: x proximity;
    padding: 6px var(--gutter) 10px;
    scroll-padding: 0 var(--gutter);
  }
  .track > :global(*) {
    scroll-snap-align: start;
  }
  @media (max-width: 560px) {
    .track {
      grid-auto-columns: var(--col-sm, 124px);
      gap: 12px;
    }
  }
  .arrow {
    position: absolute;
    top: 6px;
    bottom: 60px;
    z-index: 2;
    width: calc(var(--gutter) + 8px);
    display: grid;
    place-items: center;
    border: 0;
    color: #fff;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.2s;
  }
  .wrap:hover .arrow {
    opacity: 1;
  }
  .arrow.l {
    left: 0;
    background: linear-gradient(90deg, var(--bg), transparent);
  }
  .arrow.r {
    right: 0;
    background: linear-gradient(270deg, var(--bg), transparent);
  }
  @media (hover: none) {
    .arrow {
      display: none;
    }
  }
</style>
