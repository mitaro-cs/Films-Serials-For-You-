<script lang="ts">
  import { toggle, toggleMark, type Mark } from '$lib/marks';
  import { isFavorite, toggleFavorite } from '$lib/social/favorites';
  import type { Card, ExtCard, Kind } from '$lib/types';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';

  // Быстрые отметки поверх карточки: на компьютере - полоска снизу при наведении, на телефоне -
  // кнопка в углу (показывает текущую отметку) раскрывает ту же полоску. Слой лежит поверх
  // картинки карточки (родитель - `position: relative`, класс `tile`), в ссылку не вложен.
  let {
    card = null,
    ext = null,
    kind,
    title,
    ratio = '2 / 3',
    ondrop,
  }: {
    card?: Card | null;
    ext?: ExtCard | null;
    kind: Kind;
    title: string;
    ratio?: string;
    /** «Больше не смотрю» (крестик в ряду «Продолжить»). */
    ondrop?: () => void;
  } = $props();

  let open = $state(false);
  let popped = $state('');
  let root = $state<HTMLDivElement>();

  const mark = $derived<Mark | null>(card ? (card.status === 'dropped' ? null : card.status) : (ext?.mark === 'dropped' ? null : ext?.mark ?? null));
  const fav = $derived(card ? `t:${card.id}` : `x:${ext?.id}`);
  const ITEMS = $derived<{ id: Mark; icon: IconName; label: string }[]>([
    { id: 'want', icon: 'bookmark', label: 'Хочу' },
    { id: 'watching', icon: kind === 'series' ? 'tv' : 'play', label: 'Смотрю' },
    { id: 'watched', icon: 'check', label: 'Просмотрел' },
  ]);
  const ICON: Record<Mark, IconName> = { want: 'bookmark', watching: 'tv', watched: 'check' };

  function hit(m: Mark | 'fav') {
    popped = '';
    requestAnimationFrame(() => (popped = m));
    if (m === 'fav') toggleFavorite(fav);
    else if (card) toggle(card, m);
    else if (ext) toggleMark(ext, m);
  }

  // Открытая полоска закрывается касанием мимо.
  $effect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => !root?.contains(e.target as Node) && (open = false);
    addEventListener('pointerdown', away, true);
    return () => removeEventListener('pointerdown', away, true);
  });
</script>

<div class="qm" class:open class:marked={!!mark} style:aspect-ratio={ratio} bind:this={root}>
  <button
    class="corner"
    aria-label="Отметки: {title}"
    aria-expanded={open}
    title="Отметки"
    onclick={() => (open = !open)}
  >
    {#key mark}<span class="ic"><Icon name={mark ? ICON[mark] : 'plus'} size={14} stroke={2.6} /></span>{/key}
  </button>
  {#if ondrop}
    <button class="drop" aria-label="Больше не смотрю: {title}" title="Больше не смотрю" onclick={ondrop}>
      <Icon name="close" size={14} stroke={2.6} />
    </button>
  {/if}
  <div class="bar" role="group" aria-label="Отметить «{title}»">
    {#each ITEMS as it (it.id)}
      <button
        class:on={mark === it.id}
        class:pop={popped === it.id}
        aria-pressed={mark === it.id}
        aria-label="{it.label}: {title}"
        title={it.label}
        onclick={() => hit(it.id)}><Icon name={it.icon} size={16} stroke={2.2} /></button
      >
    {/each}
    <button
      class:on={isFavorite(fav)}
      class:pop={popped === 'fav'}
      aria-pressed={isFavorite(fav)}
      aria-label="Любимый: {title}"
      title="Любимый"
      onclick={() => hit('fav')}><Icon name="star" size={16} stroke={2.2} /></button
    >
  </div>
</div>

<style>
  .qm {
    position: absolute;
    inset: 0 0 auto 0;
    z-index: 2;
    pointer-events: none;
    border-radius: 12px;
  }
  .qm button {
    pointer-events: auto;
    display: grid;
    place-items: center;
    border: 0;
    color: #fff;
    cursor: pointer;
  }
  .corner,
  .drop {
    position: absolute;
    top: 8px;
    width: 26px;
    height: 26px;
    padding: 0;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.72);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.12);
    transition:
      opacity 0.2s,
      transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .corner {
    right: 8px;
  }
  .drop {
    left: 8px;
  }
  .corner:active,
  .drop:active {
    transform: scale(0.85);
  }
  .marked .corner {
    background: #000; outline: 1px solid #ffffff70;
    color: #fff;
  }
  .ic {
    display: grid;
    animation: tick 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes tick {
    from {
      transform: scale(0.4) rotate(-30deg);
      opacity: 0;
    }
  }

  /* Полоска - внизу картинки; выезжает снизу. */
  .bar {
    position: absolute;
    left: 50%;
    bottom: 10px;
    display: flex;
    gap: 4px;
    padding: 4px;
    border-radius: 999px;
    background: #000;
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.1),
      0 10px 24px -8px rgba(0, 0, 0, 0.8);
    opacity: 0;
    /* Спрятанная - и невидимая для вёрстки и клавиатуры (visibility гаснет после анимации). */
    visibility: hidden;
    transform: translate(-50%, 10px) scale(0.92);
    transition:
      opacity 0.2s,
      transform 0.3s cubic-bezier(0.2, 0.9, 0.3, 1.3),
      visibility 0s linear 0.3s;
    pointer-events: none;
  }
  .bar button {
    width: 32px;
    height: 32px;
    padding: 0;
    border-radius: 50%;
    background: transparent;
    pointer-events: none;
    transition:
      background 0.15s,
      color 0.15s,
      transform 0.2s;
  }
  .bar button:hover {
    background: rgba(255, 255, 255, 0.14);
  }
  .bar button.on {
    background: #000; outline: 1px solid #ffffff70;
    color: #fff;
  }
  .bar button:active {
    transform: scale(0.85);
  }
  .bar button.pop {
    animation: pop 0.45s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes pop {
    40% {
      transform: scale(1.3);
    }
  }
  .open .bar,
  .qm:focus-within .bar {
    opacity: 1;
    visibility: visible;
    transform: translate(-50%, 0) scale(1);
    transition-delay: 0s;
    pointer-events: auto;
  }
  .open .bar button,
  .qm:focus-within .bar button {
    pointer-events: auto;
  }
  @media (hover: hover) {
    /* На компьютере угол без отметки и крестик - только при наведении, полоска - тоже. */
    .corner,
    .drop {
      opacity: 0;
    }
    .marked .corner,
    :global(.tile:hover) .corner,
    :global(.tile:hover) .drop,
    .qm:focus-within .corner,
    .qm:focus-within .drop {
      opacity: 1;
    }
    :global(.tile:hover) .bar {
      opacity: 1;
      visibility: visible;
      transform: translate(-50%, 0) scale(1);
      transition-delay: 0s;
      pointer-events: auto;
    }
    :global(.tile:hover) .bar button {
      pointer-events: auto;
    }
  }
</style>
