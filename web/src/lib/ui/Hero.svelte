<script lang="ts">
  import type { Card } from '$lib/types';
  import { kindLabel, runtime } from '$lib/fmt';
  import Icon from './Icon.svelte';

  let { items }: { items: Card[] } = $props();
  let i = $state(0);
  let paused = $state(false);
  const c = $derived(items[i]);

  $effect(() => {
    if (items.length < 2 || paused) return;
    const t = setInterval(() => (i = (i + 1) % items.length), 9000);
    return () => clearInterval(t);
  });
</script>

{#if c}
  <section class="hero" aria-label="Подборка" onmouseenter={() => (paused = true)} onmouseleave={() => (paused = false)}>
    {#each items as it, k (it.id)}
      <div class="bg" class:on={k === i} style:background-image={it.backdrop ? `url(${it.backdrop})` : undefined}></div>
    {/each}
    <div class="shade"></div>
    {#key c.id}
      <div class="content">
        {#if c.logo}
          <img class="logo" src={c.logo} alt={c.title} />
        {:else}
          <h1>{c.title}</h1>
        {/if}
        <div class="meta">
          {#if c.rating}<span class="imdb">IMDb <b>{c.rating.toFixed(1)}</b></span>{/if}
          {#if c.year}<span>{c.year}</span>{/if}
          <span>{kindLabel(c.kind)}</span>
          {#if c.age}<span class="age">{c.age}</span>{/if}
          {#if c.kind === 'movie' && c.runtime}<span>{runtime(c.runtime)}</span>{/if}
          {#if c.genres.length}<span>{c.genres.slice(0, 3).join(', ')}</span>{/if}
        </div>
        {#if c.overview}<p>{c.overview}</p>{/if}
        <div class="actions">
          <a class="btn white" href="/title/{c.id}?play=1"><Icon name="play" size={18} /> Смотреть</a>
          <a class="btn" href="/title/{c.id}"><Icon name="info" size={18} /> Подробнее</a>
        </div>
      </div>
    {/key}
    {#if items.length > 1}
      <div class="dots">
        {#each items as it, k (it.id)}
          <button class:on={k === i} aria-label="Слайд {k + 1}" onclick={() => (i = k)}></button>
        {/each}
      </div>
    {/if}
  </section>
{/if}

<style>
  .hero {
    position: relative;
    height: min(86vh, 820px);
    min-height: 520px;
    display: flex;
    align-items: flex-end;
    overflow: hidden;
  }
  .bg {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center 25%;
    opacity: 0;
    transform: scale(1.06);
    transition:
      opacity 1.1s ease,
      transform 9s linear;
  }
  .bg.on {
    opacity: 1;
    transform: scale(1);
  }
  .shade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, rgba(11, 10, 26, 0.92) 0%, rgba(11, 10, 26, 0.55) 40%, rgba(11, 10, 26, 0) 70%),
      linear-gradient(0deg, var(--bg) 0%, rgba(11, 10, 26, 0) 45%),
      linear-gradient(180deg, rgba(40, 20, 90, 0.35), transparent 30%);
  }
  .content {
    position: relative;
    max-width: 640px;
    padding: 0 var(--gutter) clamp(64px, 12vh, 120px);
    animation: rise 0.7s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(18px);
    }
  }
  h1 {
    font-size: clamp(34px, 5.2vw, 64px);
    font-weight: 800;
    text-shadow: 0 4px 30px rgba(0, 0, 0, 0.5);
  }
  .logo {
    max-width: min(460px, 80%);
    max-height: 150px;
    object-fit: contain;
    object-position: left bottom;
    filter: drop-shadow(0 6px 20px rgba(0, 0, 0, 0.6));
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    margin-top: 18px;
    color: #d6d3ea;
    font-size: 14px;
  }
  .age {
    padding: 0 6px;
    border: 1px solid rgba(255, 255, 255, 0.4);
    border-radius: 4px;
    font-size: 12px;
  }
  p {
    margin: 14px 0 0;
    color: #cfcce4;
    font-size: 16px;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .actions {
    display: flex;
    gap: 12px;
    margin-top: 24px;
  }
  .actions .btn {
    height: 48px;
    padding: 0 24px;
    font-size: 15px;
  }
  .dots {
    position: absolute;
    right: var(--gutter);
    bottom: clamp(64px, 12vh, 120px);
    display: flex;
    gap: 8px;
  }
  .dots button {
    width: 8px;
    height: 8px;
    border-radius: 99px;
    border: 0;
    padding: 0;
    background: rgba(255, 255, 255, 0.35);
    cursor: pointer;
    transition: width 0.3s;
  }
  .dots button.on {
    width: 26px;
    background: #fff;
  }
  @media (max-width: 640px) {
    .hero {
      height: 78vh;
      min-height: 460px;
    }
    .dots {
      display: none;
    }
    .actions .btn {
      flex: 1;
    }
  }
</style>
