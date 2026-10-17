<script lang="ts">
  import { openStatus } from '$lib/cards.svelte';
  import { barcode, credit, day, left, length, percent, time } from '$lib/library';
  import type { Card } from '$lib/types';
  import Icon from './Icon.svelte';

  // Карточка-билет по макету: постер сверху, низ — в цвет постера, вырезы по краям на шве.
  // Просмотренное — белый билет: моноширинный шрифт, пунктир, штрихкод, дата и время.
  let { card, eager = false, index = 0 }: { card: Card; eager?: boolean; index?: number } = $props();

  const white = $derived(card.status === 'watched');
  const len = $derived(length(card));
  const who = $derived(credit(card));
  const genres = $derived(card.genres.slice(0, 3).join(' / '));
  const r = $derived(card.status === 'watching' ? card.resume : null);
  const href = $derived(r ? `/watch/${r.fileId}` : `/title/${card.id}`);
  const STATUS_ICON = { want: 'bookmark', watching: 'play', watched: 'check' } as const;

  // Переход на страницу фильма: этот постер «перелетает» на место большого (View Transitions).
  let art = $state<HTMLDivElement>();
  function fly() {
    if (!r) art?.style.setProperty('view-transition-name', `poster-${card.id}`);
  }
</script>

<article class="ticket" class:white style:--tint={card.accent ?? '#2c2c2e'} style:--i={Math.min(index, 12)}>
  <div class="art" bind:this={art}>
    {#if card.poster}
      <img src={card.poster} alt="" loading={eager ? 'eager' : 'lazy'} decoding="async" width="342" height="513" />
    {:else}
      <div class="noart"><img src="/logo.svg" alt="" width="48" height="48" /></div>
    {/if}
    {#if r}
      <div class="bar" aria-hidden="true"><i style:width="{percent(r.position, r.duration)}%"></i></div>
    {/if}
  </div>
  <div class="body">
    <h3><a {href} class="name" onclick={fly}>{card.title}</a></h3>
    {#if white}
      {#if who}<p class="line">{who}</p>{/if}
      {#if genres}<p class="line dim">{genres}</p>{/if}
      <p class="line dim">{[card.year, card.kind === 'series' ? `${card.files} сер.` : card.runtime && `${card.runtime} мин`].filter(Boolean).join(' / ')}</p>
      <div class="cols dashed">
        <span><small>Просмотрено</small>{card.watchedAt ? day(card.watchedAt) : '—'}</span>
        <span class="end"><small>Время</small>{card.watchedAt ? time(card.watchedAt) : '—'}</span>
      </div>
      <div class="code" aria-hidden="true">
        {#each barcode(card.id) as w, i (i)}<i style:flex-grow={w}></i>{/each}
      </div>
    {:else}
      {#if who}<p class="line">{who}</p>{/if}
      {#if genres}<p class="line dim">{genres}</p>{/if}
      {#if r}
        <div class="resume">
          <small>{r.label || 'Продолжить'}</small>
          <b><Icon name="play" size={12} /> {left(r.position, r.duration)}</b>
        </div>
      {:else}
        <div class="cols">
          <span><small>Добавлен</small>{day(card.addedAt)}</span>
          <span class="end"><small>{len.label}</small>{len.value}</span>
        </div>
      {/if}
    {/if}
  </div>
  <button
    class="st"
    class:on={card.status}
    aria-label="Статус: {card.status === 'want' ? 'хочу посмотреть' : card.status === 'watching' ? 'смотрю' : card.status === 'watched' ? 'просмотрено' : 'не отмечено'}"
    aria-haspopup="menu"
    onclick={(e) => openStatus(card, e.currentTarget)}
  >
    <Icon name={card.status ? STATUS_ICON[card.status] : 'plus'} size={15} stroke={2.4} />
  </button>
</article>

<style>
  .ticket {
    --notch: 9px;
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    border-radius: var(--radius);
    background: var(--tint);
    color: #fff;
    overflow: hidden;
    /* Длинные сетки: карточки вне экрана не рисуются. */
    content-visibility: auto;
    contain-intrinsic-size: auto 300px;
    transition:
      transform 0.2s cubic-bezier(0.2, 0.8, 0.2, 1),
      box-shadow 0.2s;
    /* Появление по очереди — только прозрачность и сдвиг (место занято сразу, ничего не прыгает). */
    animation: enter 0.45s calc(var(--i) * 35ms) cubic-bezier(0.2, 0.8, 0.2, 1) backwards;
  }
  @keyframes enter {
    from {
      opacity: 0;
      transform: translateY(12px) scale(0.98);
    }
  }
  .ticket:hover {
    transform: translateY(-3px);
    box-shadow: 0 18px 36px -18px rgba(0, 0, 0, 0.9);
  }
  .ticket:has(.name:focus-visible) {
    outline: 2px solid #fff;
    outline-offset: 3px;
  }

  .art {
    position: relative;
    flex: none;
    min-height: 0;
    aspect-ratio: 20 / 17;
    background: rgba(0, 0, 0, 0.35);
  }
  .art > img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 22%;
  }
  .noart {
    height: 100%;
    display: grid;
    place-items: center;
    background: linear-gradient(160deg, rgba(255, 255, 255, 0.08), rgba(0, 0, 0, 0.3));
  }
  .noart img {
    image-rendering: pixelated;
    opacity: 0.85;
  }
  .bar {
    position: absolute;
    inset: auto 0 0;
    height: 4px;
    background: rgba(0, 0, 0, 0.45);
  }
  .bar i {
    display: block;
    height: 100%;
    background: #fff;
  }

  /* Вырезы на шве — кружки цвета страницы по краям. */
  .body {
    position: relative;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 14px 12px;
  }
  .body::before,
  .body::after {
    content: '';
    position: absolute;
    top: calc(var(--notch) * -1);
    width: calc(var(--notch) * 2);
    height: calc(var(--notch) * 2);
    border-radius: 50%;
    background: var(--bg);
  }
  .body::before {
    left: calc(var(--notch) * -1);
  }
  .body::after {
    right: calc(var(--notch) * -1);
  }

  h3 {
    font-size: 16px;
    font-weight: 700;
    line-height: 1.25;
    margin-bottom: 4px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  /* Вся карточка — ссылка; кнопка статуса — поверх. */
  .name::after {
    content: '';
    position: absolute;
    inset: 0;
    z-index: 1;
  }
  .name:focus-visible {
    outline: none;
  }
  .line {
    margin: 0;
    font-size: 12px;
    line-height: 1.4;
    color: rgba(255, 255, 255, 0.82);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .line.dim {
    color: rgba(255, 255, 255, 0.6);
  }
  .cols {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    margin-top: auto;
    padding-top: 12px;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .cols span {
    display: grid;
    min-width: 0;
  }
  .cols .end {
    text-align: right;
  }
  .cols small,
  .resume small {
    font-size: 10.5px;
    color: rgba(255, 255, 255, 0.55);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .resume {
    display: grid;
    margin-top: auto;
    padding-top: 12px;
    font-size: 12px;
  }
  .resume b {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-weight: 700;
  }

  .st {
    position: absolute;
    z-index: 2;
    top: 8px;
    right: 8px;
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    color: #fff;
    cursor: pointer;
    transition:
      background 0.15s,
      transform 0.15s;
  }
  .st:hover {
    transform: scale(1.08);
  }
  .st.on {
    background: #fff;
    color: #0a0a0a;
  }

  /* ---------- белый билет: «Просмотрено» ---------- */
  .white {
    background: var(--ticket);
    color: var(--ticket-ink);
  }
  .white .body {
    font-family: var(--mono);
    padding-bottom: 20px;
  }
  .white h3 {
    font-size: 15px;
    letter-spacing: -0.01em;
    padding-bottom: 6px;
    border-bottom: 1.5px dashed rgba(0, 0, 0, 0.35);
  }
  .white .line {
    color: rgba(0, 0, 0, 0.75);
    font-size: 11.5px;
  }
  .white .line.dim {
    color: rgba(0, 0, 0, 0.55);
  }
  .white .cols small {
    color: rgba(0, 0, 0, 0.5);
    font-style: italic;
  }
  .dashed {
    border-top: 1.5px dashed rgba(0, 0, 0, 0.35);
    margin-top: 8px;
    padding-top: 6px;
  }
  .code {
    display: flex;
    gap: 2px;
    height: 22px;
    margin-top: 10px;
  }
  .code i {
    flex-basis: 0;
    background: var(--ticket-ink);
  }
  .code i:nth-child(3n) {
    background: transparent;
  }
  /* Нижний край — перфорация, как у отрывного билета. */
  .white::after {
    content: '';
    position: absolute;
    inset: auto 0 -6px;
    height: 12px;
    background: radial-gradient(circle at 8px 6px, var(--bg) 5px, transparent 5.5px) 0 0 / 16px 12px repeat-x;
    pointer-events: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .ticket {
      animation: none;
    }
    .ticket:hover {
      transform: none;
    }
  }
</style>
