<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { toggleList } from '$lib/cards.svelte';
  import type { Card } from '$lib/types';
  import Icon from './Icon.svelte';

  // Герой как у Apple TV: большой кадр на всю ширину, слева крупное название, жанр, возраст,
  // описание и кнопка; внизу точки. Листается сам раз в 8 с (пауза под мышью, в фоне и при
  // «меньше движения»), пальцем — свайпом. Высота задана заранее: ничего не сдвигается.
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
        in:fade={{ duration: still ? 0 : 700 }}
        out:fade={{ duration: still ? 0 : 700 }}
      />
    {/key}
  </div>

  {#key cur.id}
    <div class="info" in:fly={{ y: still ? 0 : 14, duration: 500, delay: still ? 0 : 150 }}>
      {#if cur.logo}
        <h1 class="logo"><img src={cur.logo} alt={cur.title} decoding="async" /></h1>
      {:else}
        <h1 class:short class:long>{cur.title}</h1>
      {/if}
      <p class="meta">
        {#if cur.genres[0]}<span>{cur.genres[0]}</span>{/if}
        {#if cur.year}<span>{cur.year}</span>{/if}
        {#if cur.age}<span class="age">{cur.age}</span>{/if}
        {#if cur.kind === 'series' && cur.seasons}<span>{cur.seasons} сез.</span>{/if}
      </p>
      {#if cur.overview}<p class="text">{cur.overview}</p>{/if}
      <div class="acts">
        {#if cur.status === 'watching' && cur.resume}
          <a class="go" href="/watch/{cur.resume.fileId}"><Icon name="play" size={18} /> Продолжить</a>
        {/if}
        <a class="go" class:alt={cur.status === 'watching' && cur.resume} href="/title/{cur.id}">{kind}</a>
        <button
          class="round"
          aria-label={cur.inList ? 'Убрать из «Хочу посмотреть»' : 'Хочу посмотреть'}
          title={cur.inList ? 'В списке' : 'Хочу посмотреть'}
          onclick={() => toggleList(cur)}
        >
          <Icon name={cur.inList ? 'check' : 'plus'} size={20} />
        </button>
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
  .hero {
    position: relative;
    height: clamp(460px, 78vh, 860px);
    display: flex;
    align-items: flex-end;
    padding: 0 var(--gutter) clamp(150px, 19vh, 210px);
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
    object-position: center 30%;
  }
  .bg img.poster {
    object-fit: contain;
    object-position: 85% center;
  }
  /* Слева темнее — текст читается; снизу — в цвет страницы, ряды «лежат» на кадре. */
  .bg::after {
    content: '';
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, var(--bg) 0%, rgba(0, 0, 0, 0.35) 34%, transparent 62%),
      linear-gradient(90deg, color-mix(in srgb, var(--tint) 45%, rgba(0, 0, 0, 0.7)) 0%, transparent 58%);
  }
  .info {
    max-width: min(560px, 100%);
    display: grid;
    gap: 10px;
  }
  h1 {
    font-size: clamp(36px, 5.6vw, 84px);
    font-weight: 900;
    line-height: 0.98;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    text-shadow: 0 4px 30px rgba(0, 0, 0, 0.35);
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  h1.long {
    font-size: clamp(30px, 3.6vw, 56px);
  }
  /* Короткое название — вразрядку, как «S I L O». */
  h1.short {
    letter-spacing: 0.22em;
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
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.85);
  }
  .meta span + span:not(.age)::before {
    content: '·';
    margin-right: 8px;
    color: rgba(255, 255, 255, 0.5);
  }
  .age {
    padding: 0 5px;
    border: 1.5px solid rgba(255, 255, 255, 0.6);
    border-radius: 4px;
    font-size: 11px;
    line-height: 16px;
  }
  .text {
    margin: 0;
    font-size: clamp(15px, 1.15vw, 18px);
    line-height: 1.4;
    color: rgba(255, 255, 255, 0.92);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .acts {
    display: flex;
    gap: 10px;
    margin-top: 8px;
  }
  /* Кнопка — светлое стекло в цвет кадра, как «Go to Show». */
  .go {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 46px;
    padding: 0 30px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--tint) 30%, rgba(255, 255, 255, 0.34));
    color: #fff;
    font-size: 16px;
    font-weight: 600;
    white-space: nowrap;
    transition:
      transform 0.2s,
      background 0.2s;
  }
  .go:not(.alt):first-child {
    background: rgba(255, 255, 255, 0.92);
    color: #111;
  }
  .go:hover {
    transform: scale(1.04);
  }
  .round {
    display: grid;
    place-items: center;
    width: 46px;
    height: 46px;
    border-radius: 50%;
    border: 0;
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
    cursor: pointer;
    transition: transform 0.2s;
  }
  .round:hover {
    transform: scale(1.08);
  }
  .dots {
    position: absolute;
    left: 50%;
    bottom: clamp(118px, 15vh, 170px);
    translate: -50% 0;
    display: flex;
    gap: 2px;
  }
  .dots button {
    width: 22px;
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
    width: 7px;
    height: 7px;
    border-radius: 99px;
    background: rgba(255, 255, 255, 0.4);
    transition:
      transform 0.3s,
      background 0.3s;
  }
  .dots button.on::after {
    background: #fff;
    transform: scaleX(2.6);
  }
  @media (max-width: 640px) {
    .hero {
      height: clamp(440px, 74vh, 640px);
      padding-bottom: 104px;
    }
    .bg::after {
      background: linear-gradient(0deg, var(--bg) 0%, rgba(0, 0, 0, 0.55) 45%, transparent 75%);
    }
    .info {
      justify-items: center;
      text-align: center;
      margin: 0 auto;
    }
    .meta {
      justify-content: center;
    }
    .text {
      -webkit-line-clamp: 2;
      line-clamp: 2;
    }
    .go {
      padding: 0 22px;
    }
    .dots {
      bottom: 70px;
    }
  }
</style>
