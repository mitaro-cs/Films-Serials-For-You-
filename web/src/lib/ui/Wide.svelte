<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import { caption, percent } from '$lib/library';
  import type { Card } from '$lib/types';
  import { placeholder } from './art';
  import Icon from './Icon.svelte';

  // Широкая карточка как у Apple TV: кадр 16:9 со скруглением, под ним — название и подпись
  // («Недавно добавлено», где остановился, статус). Размер задан заранее — ничего не прыгает.
  let { card, eager = false, i = null }: { card: Card; eager?: boolean; i?: number | null } = $props();

  const src = $derived(card.backdropSmall ?? card.backdrop ?? card.poster);
  const r = $derived(card.status === 'watching' ? card.resume : null);
</script>

<a class="wide" class:rise={i !== null} style:--i={i} href="/title/{card.id}" style:--tint={card.accent ?? '#2c2c2e'}>
  <span class="art" style:background={src ? undefined : placeholder(card.title)}>
    {#if src}
      <img use:fadeIn {src} alt="" loading={eager ? 'eager' : 'lazy'} decoding="async" width="640" height="360" class:poster={!card.backdropSmall && !card.backdrop} />
    {/if}
    {#if card.logo}
      <img use:fadeIn class="mark" src={card.logo} alt="" loading="lazy" decoding="async" />
    {:else if !src}
      <span class="ph">{card.title}</span>
    {/if}
    {#if card.status === 'watched'}<span class="done" title="Просмотрено"><Icon name="check" size={14} stroke={3} /></span>{/if}
    {#if r}<span class="bar" aria-hidden="true"><i style:width="{percent(r.position, r.duration)}%"></i></span>{/if}
  </span>
  <span class="name">{card.title}</span>
  <span class="sub">{caption(card)}</span>
</a>

<style>
  .wide {
    display: grid;
    gap: 2px;
    min-width: 0;
    color: var(--text);
  }
  .art {
    position: relative;
    display: block;
    aspect-ratio: 16 / 9;
    margin-bottom: 8px;
    border-radius: 12px;
    overflow: hidden;
    background: var(--tint);
    box-shadow: 0 10px 30px -18px rgba(0, 0, 0, 0.9);
    transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .wide:hover .art,
  .wide:focus-visible .art {
    transform: scale(1.04);
  }
  .wide:focus-visible {
    outline: none;
  }
  .wide:focus-visible .art {
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
  /* Без кадра — постер по центру поверх его же цвета. */
  .art img.poster {
    object-position: center 20%;
  }
  .art .mark {
    inset: auto 10% 10%;
    width: 80%;
    height: 34%;
    object-fit: contain;
    object-position: center bottom;
    filter: drop-shadow(0 2px 8px rgba(0, 0, 0, 0.6));
  }
  .ph {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 12px;
    text-align: center;
    font-weight: 800;
    font-size: 18px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }
  .done {
    position: absolute;
    top: 8px;
    right: 8px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.92);
    color: #000;
  }
  .bar {
    position: absolute;
    inset: auto 10px 10px;
    height: 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: #fff;
  }
  .name,
  .sub {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name {
    font-size: 15px;
    font-weight: 500;
  }
  .sub {
    font-size: 13.5px;
    color: rgba(255, 255, 255, 0.5);
  }
</style>
