<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fadeIn } from '$lib/ui/fade';
  import { fitTitle } from '$lib/ui/fitTitle';
  import { artSet, artW, placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';

  // Верх страницы фильма на компьютере - по макету Rewind: кадр из фильма на весь экран, слева
  // постер с «Смотреть», рядом название, «год | длительность», слоган, описание и жанры, справа -
  // стеклянная плашка действий. Размеры - в долях `--s`: кадр и плашка растут и уменьшаются вместе,
  // плашка всегда занимает ту же часть кадра (макет считан на 1440 × 900).
  let {
    title,
    art,
    poster = null,
    tall = false,
    meta = [],
    age = null,
    tagline = null,
    overview = '',
    genres = [],
    play = null,
    action,
    tools,
    under,
    panel,
  }: {
    title: string;
    art: string | null;
    poster?: string | null;
    /** Кадра нет, вместо него - постер (размытый). */
    tall?: boolean;
    meta?: string[];
    age?: string | null;
    tagline?: string | null;
    overview?: string;
    genres?: string[];
    play?: { label: string; href: string } | null;
    /** Своя главная кнопка (у фильма из ленты - найти видео, смотреть на сайте): класс `rw-watch`. */
    action?: Snippet;
    /** Кнопки хоста над кадром («Обложка», «Фон»). */
    tools?: Snippet;
    /** Под описанием: где остановились, друзья. */
    under?: Snippet;
    /** Содержимое правой плашки. */
    panel: Snippet;
  } = $props();

  // Кадр - в полном разрешении: IMDb отдаёт его набором ширин, свой - 1920 px.
  const set = $derived(artSet(art));
</script>

<section class="rwh rw">
  <div class="shot" aria-hidden="true">
    {#key art}
      {#if art}<img use:fadeIn class:tall src={art} srcset={set} sizes="100vw" alt="" decoding="async" fetchpriority="high" />{:else}<div class="ph" style:background={placeholder(title)}></div>{/if}
    {/key}
  </div>
  <div class="veil" aria-hidden="true"></div>
  {#if tools}<div class="tools">{@render tools()}</div>{/if}

  <div class="stage">
    <div class="poster" style:background={poster ? undefined : placeholder(title)}>
      {#if poster}<img use:fadeIn src={artW(poster, 640)} alt={title} width="460" height="690" decoding="async" />{:else}<span>{title}</span>{/if}
    </div>
    <div class="text">
      <h1 use:fitTitle={title}>{title}</h1>
      {#if meta.length || age}
        <p class="meta">
          {#each meta as m, i (i)}{#if i}<i aria-hidden="true">|</i>{/if}<span>{m}</span>{/each}
          {#if age}<b class="age">{age}</b>{/if}
        </p>
      {/if}
      {#if tagline}<p class="tag">{tagline}</p>{/if}
      {#if overview}<p class="ov selectable">{overview}</p>{/if}
      {@render under?.()}
    </div>
    <aside class="plate" aria-label="Действия">{@render panel()}</aside>
    <div class="go">
      {#if action}{@render action()}{:else if play}<a class="watch" href={play.href}><Icon name="play" size={18} /> {play.label}</a>{/if}
    </div>
    <div class="pills">
      {#each genres.slice(0, 4) as g (g)}<a class="pill" href="/search?q={encodeURIComponent(g)}">{g}</a>{/each}
    </div>
  </div>
</section>

<style>
  /* 1 s = 1 px макета при ширине 1440 и высоте 900; меньше окно - всё пропорционально меньше. */
  .rwh {
    /* 0.82: меньше места под текст и кнопки - больше под сам кадр, как на макете. */
    --s: calc(min(calc(100vw / 1440), calc(100svh / 900)) * 0.82);
    position: relative;
    height: 100svh;
    min-height: 560px;
    overflow: hidden;
    isolation: isolate;
    background: #0b0b0b;
  }
  .shot,
  .veil {
    position: absolute;
    inset: 0;
  }
  .shot {
    z-index: -2;
  }
  .shot img,
  .shot .ph {
    width: 100%;
    height: 100%;
    max-width: none;
    object-fit: cover;
    object-position: center 28%;
    animation: shot-in 1.2s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .shot img.tall {
    filter: blur(6px);
    transform: scale(1.06);
  }
  /* Низ уходит в цвет листа ниже, верх - чуть темнее под шапкой, слева - под текстом. */
  .veil {
    z-index: -1;
    pointer-events: none;
    background:
      linear-gradient(0deg, #1b1b1b 0%, #0e0e0ee6 16%, #0e0e0e59 42%, transparent 66%),
      linear-gradient(90deg, #0000008c 0%, #00000040 46%, transparent 72%),
      linear-gradient(180deg, #0000008c 0%, transparent 22%);
  }
  .tools {
    position: absolute;
    z-index: 3;
    top: calc(var(--header) + 14px);
    right: var(--gutter);
    display: flex;
    gap: 8px;
  }
  @keyframes shot-in {
    from {
      opacity: 0;
      transform: scale(1.05);
    }
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(calc(18 * var(--s)));
    }
  }

  .stage {
    position: absolute;
    left: calc(119 * var(--s));
    right: calc(121 * var(--s));
    bottom: calc(86 * var(--s));
    display: grid;
    grid-template-columns: calc(275 * var(--s)) minmax(0, calc(558 * var(--s))) calc(250 * var(--s));
    grid-template-areas:
      'poster text plate'
      'go pills .';
    justify-content: space-between;
    align-items: end;
  }
  .poster {
    grid-area: poster;
    aspect-ratio: 2 / 3;
    border-radius: calc(14 * var(--s));
    overflow: hidden;
    display: grid;
    align-items: end;
    box-shadow: 0 calc(30 * var(--s)) calc(60 * var(--s)) calc(-20 * var(--s)) #000c;
    animation: rise 0.7s 0.1s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .poster span {
    padding: 16px;
    font-weight: 800;
    font-size: 20px;
  }

  .text {
    grid-area: text;
    min-width: 0;
    align-self: end;
    padding-bottom: calc(33 * var(--s));
    animation: rise 0.7s 0.2s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  h1 {
    margin: 0;
    font-size: max(30px, calc(57 * var(--s)));
    line-height: 1.06;
    font-weight: 700;
    letter-spacing: -0.02em;
    white-space: nowrap;
    text-overflow: ellipsis;
    overflow: hidden;
    text-shadow: 0 2px 28px #0008;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0 calc(10 * var(--s));
    margin: calc(16 * var(--s)) 0 0;
    font-size: max(13px, calc(20 * var(--s)));
    color: #e9e9ec;
  }
  .meta i {
    font-style: normal;
    opacity: 0.55;
  }
  .age {
    padding: 0 calc(9 * var(--s));
    border: 1px solid #ffffff8c;
    border-radius: 999px;
    font-size: 0.8em;
    font-weight: 500;
  }
  .tag {
    margin: calc(34 * var(--s)) 0 0;
    font-size: max(13px, calc(20 * var(--s)));
    color: #f1f1f3;
  }
  .ov {
    margin: calc(12 * var(--s)) 0 0;
    font-size: max(13px, calc(19 * var(--s)));
    line-height: 1.5;
    color: #cfcfd4;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .go {
    grid-area: go;
    margin-top: calc(31 * var(--s));
    animation: rise 0.7s 0.3s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .watch,
  .go :global(.rw-watch) {
    width: 100%;
    border: 0;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: max(40px, calc(58 * var(--s)));
    padding: 6px calc(14 * var(--s));
    box-sizing: border-box;
    text-align: center;
    line-height: 1.15;
    border-radius: 999px;
    background: #fff;
    color: #0a0a0a;
    font-size: max(13px, calc(18 * var(--s)));
    font-weight: 600;
    transition: transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.4);
  }
  .watch:hover,
  .go :global(.rw-watch:hover:not(:disabled)) {
    transform: translateY(-2px) scale(1.02);
  }
  .watch:active,
  .go :global(.rw-watch:active:not(:disabled)) {
    transform: scale(0.97);
  }
  .go :global(.rw-watch:disabled) {
    opacity: 0.6;
    cursor: default;
  }
  .pills {
    grid-area: pills;
    display: flex;
    flex-wrap: nowrap;
    gap: calc(11 * var(--s));
    margin-top: calc(31 * var(--s));
    overflow: hidden;
    animation: rise 0.7s 0.3s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    height: max(38px, calc(58 * var(--s)));
    padding: 0 calc(24 * var(--s));
    border-radius: 999px;
    background: #2b2b2bd9;
    font-size: max(13px, calc(18 * var(--s)));
    font-weight: 500;
    white-space: nowrap;
    transition:
      background 0.2s,
      transform 0.2s;
  }
  .pill:hover {
    background: #3a3a3af2;
    transform: translateY(-2px);
  }

  /* Правая плашка: стекло, три действия и оценка; ниже - «В списки» и «Отзыв». */
  .plate {
    grid-area: plate;
    align-self: end;
    display: grid;
    gap: calc(14 * var(--s));
    margin-bottom: calc(20 * var(--s));
    animation: rise 0.7s 0.4s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .plate :global(.rw-box) {
    display: grid;
    gap: calc(10 * var(--s));
    padding: calc(14 * var(--s)) calc(10 * var(--s)) calc(16 * var(--s));
    border-radius: calc(24 * var(--s));
    background: #26262685;
    backdrop-filter: blur(18px);
    box-shadow: inset 0 0 0 1px #ffffff14;
  }
  .plate :global(.rw-tri) {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
  }
  .plate :global(.rw-tri button) {
    display: grid;
    justify-items: center;
    gap: calc(8 * var(--s));
    padding: calc(10 * var(--s)) 0;
    border: 0;
    border-radius: calc(16 * var(--s));
    background: none;
    color: #b5b5ba;
    font: inherit;
    font-size: max(11px, calc(14 * var(--s)));
    cursor: pointer;
    transition:
      color 0.15s,
      background 0.15s,
      transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.4);
  }
  .plate :global(.rw-tri button:hover:not(:disabled)) {
    color: #fff;
    background: #ffffff0f;
  }
  .plate :global(.rw-tri button:active:not(:disabled)) {
    transform: scale(0.92);
  }
  .plate :global(.rw-tri button.on) {
    color: #fff;
  }
  .plate :global(.rw-tri button.on svg) {
    fill: currentColor;
  }
  .plate :global(.rw-tri button:disabled) {
    opacity: 0.4;
    cursor: default;
  }
  .plate :global(.rw-rate) {
    display: grid;
    justify-items: center;
    gap: calc(6 * var(--s));
    color: #9a9aa0;
    font-size: max(11px, calc(14 * var(--s)));
  }
  .plate :global(.rw-btn) {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: max(38px, calc(56 * var(--s)));
    padding: 0 calc(14 * var(--s));
    border: 0;
    border-radius: 999px;
    background: #2e2e2eb3;
    backdrop-filter: blur(14px);
    box-shadow: inset 0 0 0 1px #ffffff12;
    color: #f3f3f5;
    font: inherit;
    font-size: max(13px, calc(18 * var(--s)));
    font-weight: 500;
    cursor: pointer;
    transition:
      background 0.2s,
      transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.4);
  }
  /* «Смотреть вместе» - на виду, рядом с оценкой: светлая обводка отличает её от остальных капсул. */
  .plate :global(.rw-btn.together) {
    background: #ffffff1f;
    box-shadow: inset 0 0 0 1.5px #ffffffb8;
    font-weight: 600;
  }
  .plate :global(.rw-btn:hover) {
    background: #444444d9;
    transform: translateY(-2px);
  }
  .plate :global(.rw-btn:active) {
    transform: scale(0.97);
  }
  .plate :global(.rw-row) {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: calc(10 * var(--s));
  }
  .plate :global(.rw-btn.round) {
    width: max(40px, calc(58 * var(--s)));
    padding: 0;
  }

  :global(.no-motion) :is(.shot img, .shot .ph, .poster, .text, .go, .pills, .plate) {
    animation: none;
  }
</style>
