<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import type { Card, ExtCard } from '$lib/types';
  import { artW, placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import QuickMarks from '$lib/ui/QuickMarks.svelte';
  import { remember } from './sheet.svelte';

  // Постер 2:3: название, год и оценка, поверх - быстрые отметки. Из ленты IMDb - ссылка на его
  // страницу `/ext/<id>` (карточка запоминается - страница показывается сразу), с сервиса
  // (`local`) - на `/title/<id>`.
  let {
    card,
    local = null,
    rank = null,
    i = null,
    marks = true,
  }: { card?: ExtCard; local?: Card | null; rank?: number | null; i?: number | null; marks?: boolean } = $props();

  const c = $derived(
    local
      ? { title: local.title, year: local.year, rating: local.rating, poster: local.poster, here: false, kind: local.kind }
      : { title: card!.title, year: card!.year, rating: card!.rating, poster: card!.poster, here: !!card!.local, kind: card!.kind },
  );
  let replacement = $state<string | null>(null);
  let loaded = $state(false);
  const source = $derived(replacement ?? c.poster);
  $effect(() => { void c.poster; void card?.id; replacement = null; loaded = false; });
  function retryImage() {
    const fallback = card ? `/api/discover/poster/${card.id}` : null;
    if (fallback && source !== fallback) replacement = fallback;
    else loaded = false;
  }
</script>

{#snippet body()}
  <span class="art" class:ranked={rank !== null} style:background={c.poster ? undefined : placeholder(c.title)}>
    {#if !loaded}<span class="ph">{c.title}</span>{/if}
    {#if source}<img use:fadeIn src={artW(source, 342)} alt="" loading="lazy" decoding="async" width="342" height="513" onload={() => loaded = true} onerror={retryImage} />{/if}
    {#if rank}<span class="rank">{rank}</span>{/if}
    {#if c.here}<span class="here">Есть у нас</span>{/if}
  </span>
  <span class="name">{c.title}</span>
  <span class="sub">
    {#if c.year}{c.year}{/if}{#if c.rating}<span class="star"><Icon name="star" size={11} /> {c.rating.toFixed(1)}</span>{/if}
  </span>
{/snippet}

<div class="tile" class:rise={i !== null} style:--i={i}>
  {#if local}
    <a class="poster" href="/title/{local.id}" aria-label="{c.title}{c.year ? `, ${c.year}` : ''}">{@render body()}</a>
  {:else}
    <a class="poster" href="/ext/{card!.id}" onclick={() => remember(card!)} aria-label="{c.title}{c.year ? `, ${c.year}` : ''}">{@render body()}</a>
  {/if}
  {#if marks}<QuickMarks card={local} ext={local ? null : card} kind={c.kind} title={c.title} />{/if}
</div>

<style>
  .tile {
    position: relative;
    min-width: 0;
  }
  .poster {
    display: grid;
    gap: 2px;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .art {
    position: relative;
    display: block;
    aspect-ratio: 2 / 3;
    margin-bottom: 10px;
    border-radius: 14px;
    overflow: hidden;
    background: var(--surface);
    box-shadow: 0 14px 30px -16px #000;
    transition: transform 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .tile:hover .art,
  .poster:focus-visible .art {
    transform: scale(1.04);
  }
  .poster:focus-visible {
    outline: none;
  }
  .poster:focus-visible .art {
    outline: 3px solid #fff;
    outline-offset: 2px;
  }
  .art img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .ph {
    position: absolute;
    inset: 40px 10px 10px;
    display: grid;
    place-items: center;
    padding: 0;
    text-align: center;
    font-weight: 700;
    overflow: hidden;
  }
  .ranked .ph { bottom: 44px; }
  .rank {
    position: absolute;
    left: 8px;
    bottom: 6px;
    font-size: 30px;
    font-weight: 800;
    line-height: 1;
    color: #fff;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.8);
  }
  .here {
    position: absolute;
    top: 8px;
    left: 8px;
    padding: 2px 8px;
    border-radius: 6px;
    background: #fff;
    color: #000;
    font-size: 11px;
    font-weight: 700;
  }
  .name,
  .sub {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name {
    font-size: 14px;
    font-weight: 700;
  }
  .sub {
    display: flex;
    gap: 8px;
    font-size: 13px;
    color: var(--muted);
  }
  .star {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--imdb);
  }
</style>
