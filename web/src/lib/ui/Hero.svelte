<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import type { Card } from '$lib/types';
  import Icon from './Icon.svelte';

  // Герой как у Apple TV: кадр на весь экран, слева крупное название, жанр, возраст, описание
  // и одна кнопка «К фильму»; первый ряд лежит на нижней части кадра, точки — над ним. Листается
  // сам раз в 8 с (пауза под мышью, в фоне и при «меньше движения»), пальцем — свайпом. Высота
  // задана заранее: ничего не сдвигается. Цвет кадра красит капсулу разделов в шапке.
  let { items }: { items: Card[] } = $props();

  let at = $state(0);
  let paused = $state(false);
  const cur = $derived(items[Math.min(at, items.length - 1)]);
  const still = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  $effect(() => {
    if (items.length < 2 || paused || still) return;
    const t = setInterval(() => {
      if (document.visibilityState === 'visible') at = (at + 1) % items.length;
    }, 8000);
    return () => clearInterval(t);
  });

  $effect(() => {
    document.documentElement.style.setProperty('--hero-tint', cur.accent ?? '#3a2a1f');
    return () => document.documentElement.style.removeProperty('--hero-tint');
  });

  // Следующий кадр — заранее, чтобы смена была без вспышки.
  $effect(() => {
    const next = items[(at + 1) % items.length];
    if (next?.backdrop) new Image().src = next.backdrop;
  });

  function go(d: number) {
    at = (at + d + items.length) % items.length;
  }

  let x0 = 0;
  function down(e: PointerEvent) {
    if (e.pointerType === 'touch') x0 = e.clientX;
  }
  function up(e: PointerEvent) {
    if (e.pointerType !== 'touch' || !x0) return;
    const dx = e.clientX - x0;
    x0 = 0;
    if (Math.abs(dx) > 50) go(dx < 0 ? 1 : -1);
  }

  const short = $derived(cur.title.length <= 12);
  const long = $derived(cur.title.length > 22);
  const kind = $derived(cur.kind === 'series' ? 'К сериалу' : 'К фильму');
</script>

<section
  class="hero"
  aria-roledescription="карусель"
  aria-label="Рекомендуем"
  style:--tint={cur.accent ?? '#3a2a1f'}
  onmouseenter={() => (paused = true)}
  onmouseleave={() => (paused = false)}
  onfocusin={() => (paused = true)}
  onfocusout={() => (paused = false)}
  onpointerdown={down}
  onpointerup={up}
>
  <div class="bg" aria-hidden="true">
    {#key cur.id}
      <img
        src={cur.backdrop ?? cur.posterLarge ?? cur.poster}
        alt=""
        decoding="async"
        fetchpriority={at === 0 ? 'high' : 'auto'}
        class:poster={!cur.backdrop}
        in:fade={{ duration: still ? 0 : 450 }}
        out:fade={{ duration: still ? 0 : 450 }}
      />
    {/key}
  </div>

  {#key cur.id}
    <div class="info" in:fly={{ y: still ? 0 : 10, duration: 400, delay: still ? 0 : 100 }}>
      {#if cur.logo}
        <h1 class="logo"><img src={cur.logo} alt={cur.title} decoding="async" /></h1>
      {:else}
        <h1 class:short class:long>{cur.title}</h1>
      {/if}
      <p class="meta">
        {#if cur.genres[0]}<span>{cur.genres[0]}</span>{/if}
        {#if cur.age}<span class="age">{cur.age}</span>{/if}
      </p>
      {#if cur.overview}<p class="text">{cur.overview}</p>{/if}
      <div class="acts">
        {#if cur.status === 'watching' && cur.resume}
          <a class="go" href="/watch/{cur.resume.fileId}"><Icon name="play" size={18} /> Продолжить</a>
        {:else}
          <a class="go" href="/title/{cur.id}">{kind}</a>
        {/if}
      </div>
    </div>
  {/key}

  {#if items.length > 1}
    <div class="dots" role="tablist" aria-label="Кадры">
      {#each items as c, i (c.id)}
        <button role="tab" aria-selected={i === at} aria-label={c.title} class:on={i === at} onclick={() => (at = i)}></button>
      {/each}
    </div>
  {/if}
</section>

<style>
  /* Кадр на весь экран; снизу место под первый ряд (--row) и точки. */
  .hero {
    position: relative;
    height: clamp(560px, 100svh, 1200px);
    display: flex;
    align-items: flex-end;
    padding: 0 var(--gutter) calc(var(--row) + clamp(60px, 9vh, 110px));
    overflow: hidden;
    isolation: isolate;
    touch-action: pan-y;
  }
  .bg {
    position: absolute;
    inset: 0;
    z-index: -1;
    background: var(--tint);
  }
  .bg img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 25%;
    /* Кадр медленно «отъезжает», как у Apple TV (только transform). */
    animation: kenburns 12s ease-out both;
  }
  @keyframes kenburns {
    from {
      transform: scale(1.08);
    }
  }
  .bg img.poster {
    object-fit: contain;
    object-position: 85% center;
  }
  /* Как у Apple TV: кадр почти не затемнён, только слева под текстом и внизу под рядом. */
  .bg::after {
    content: '';
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, var(--bg) 0%, rgba(0, 0, 0, 0.62) 18%, rgba(0, 0, 0, 0.18) 42%, transparent 58%),
      linear-gradient(90deg, rgba(0, 0, 0, 0.45) 0%, rgba(0, 0, 0, 0.12) 38%, transparent 60%);
  }
  .info {
    max-width: min(440px, 100%);
    display: grid;
    gap: 8px;
  }
  h1 {
    font-size: clamp(40px, 6vw, 96px);
    font-weight: 800;
    line-height: 0.98;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    text-shadow: 0 2px 24px rgba(0, 0, 0, 0.3);
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  h1.long {
    font-size: clamp(30px, 3.4vw, 52px);
  }
  /* Короткое название — широко вразрядку, как «S I L O». */
  h1.short {
    letter-spacing: 0.32em;
  }
  /* Высота логотипа задана заранее — после загрузки картинки текст ниже не сдвинется. */
  h1.logo {
    height: clamp(80px, 11vw, 150px);
  }
  h1.logo img {
    width: min(420px, 80vw);
    height: 100%;
    object-fit: contain;
    object-position: left bottom;
    filter: drop-shadow(0 4px 24px rgba(0, 0, 0, 0.45));
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 2px 0 0;
    font-size: 15px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.8);
  }
  .age {
    padding: 0 4px;
    border: 1.5px solid rgba(255, 255, 255, 0.7);
    border-radius: 3px;
    font-size: 10.5px;
    font-weight: 700;
    line-height: 15px;
  }
  .text {
    margin: 4px 0 0;
    font-size: clamp(15px, 1.15vw, 18px);
    line-height: 1.4;
    color: #fff;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .acts {
    display: flex;
    gap: 10px;
    margin-top: 14px;
  }
  /* «Go to Show»: светлое полупрозрачное стекло в цвет кадра, белый текст. */
  .go {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-width: 170px;
    height: 46px;
    padding: 0 28px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--tint) 25%, rgba(255, 255, 255, 0.36));
    color: #fff;
    font-size: 17px;
    font-weight: 600;
    white-space: nowrap;
    transition:
      transform 0.2s,
      background 0.2s;
  }
  .go:hover,
  .go:focus-visible {
    transform: scale(1.05);
    background: rgba(255, 255, 255, 0.9);
    color: #111;
    outline: none;
  }
  .dots {
    position: absolute;
    left: 50%;
    bottom: calc(var(--row) + 14px);
    translate: -50% 0;
    display: flex;
    gap: 2px;
  }
  .dots button {
    width: 18px;
    height: 22px;
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    display: grid;
    place-items: center;
  }
  .dots button::after {
    content: '';
    width: 6px;
    height: 6px;
    border-radius: 99px;
    background: rgba(255, 255, 255, 0.4);
    transition:
      transform 0.3s,
      background 0.3s;
  }
  .dots button.on::after {
    background: rgba(255, 255, 255, 0.95);
    transform: scaleX(3);
  }
  @media (max-width: 640px) {
    .hero {
      height: clamp(520px, 86svh, 760px);
      padding-bottom: calc(var(--row) + 54px);
    }
    .bg::after {
      background: linear-gradient(0deg, var(--bg) 0%, rgba(0, 0, 0, 0.6) 35%, transparent 65%);
    }
    .info {
      justify-items: center;
      text-align: center;
      margin: 0 auto;
    }
    .meta {
      justify-content: center;
    }
    .dots {
      bottom: calc(var(--row) + 8px);
    }
  }
</style>
