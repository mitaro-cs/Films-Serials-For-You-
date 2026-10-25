<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import type { Card, ExtCard } from '$lib/types';
  import { placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import { remember } from './sheet.svelte';

  // Постер 2:3: название, год и оценка. Из ленты IMDb — ссылка на его страницу `/ext/<id>`
  // (карточка запоминается — страница показывается сразу), с сервиса (`local`) — на `/title/<id>`.
  let {
    card,
    local = null,
    rank = null,
    i = null,
  }: { card?: ExtCard; local?: Card | null; rank?: number | null; i?: number | null } = $props();

  const c = $derived(
    local
      ? { title: local.title, year: local.year, rating: local.rating, poster: local.poster, here: false, mark: null }
      : { title: card!.title, year: card!.year, rating: card!.rating, poster: card!.poster, here: !!card!.local, mark: card!.mark },
  );
</script>

{#snippet body()}
  <span class="art" style:background={c.poster ? undefined : placeholder(c.title)}>
    {#if c.poster}<img use:fadeIn src={c.poster} alt="" loading="lazy" decoding="async" width="342" height="513" />{:else}<span class="ph">{c.title}</span>{/if}
    {#if rank}<span class="rank">{rank}</span>{/if}
    {#if c.here}<span class="here">Есть у нас</span>{/if}
    {#if c.mark}<span class="mark" title={c.mark === 'watched' ? 'Просмотрено' : 'Буду смотреть'}><Icon name={c.mark === 'watched' ? 'check' : 'bookmark'} size={13} stroke={2.6} /></span>{/if}
  </span>
  <span class="name">{c.title}</span>
  <span class="sub">
    {#if c.year}{c.year}{/if}{#if c.rating}<span class="star"><Icon name="star" size={11} /> {c.rating.toFixed(1)}</span>{/if}
  </span>
{/snippet}

{#if local}
  <a class="poster" class:rise={i !== null} style:--i={i} href="/title/{local.id}" aria-label="{c.title}{c.year ? `, ${c.year}` : ''}">{@render body()}</a>
{:else}
  <a class="poster" class:rise={i !== null} style:--i={i} href="/ext/{card!.id}" onclick={() => remember(card!)} aria-label="{c.title}{c.year ? `, ${c.year}` : ''}">{@render body()}</a>
{/if}

<style>
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
    margin-bottom: 8px;
    border-radius: 12px;
    overflow: hidden;
    background: var(--surface);
    transition: transform 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .poster:hover .art,
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
    inset: 0;
    display: grid;
    place-items: center;
    padding: 10px;
    text-align: center;
    font-weight: 700;
    overflow: hidden;
  }
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
  .mark {
    position: absolute;
    top: 8px;
    right: 8px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.7);
    color: #fff;
  }
  .name,
  .sub {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name {
    font-size: 14px;
    font-weight: 500;
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
