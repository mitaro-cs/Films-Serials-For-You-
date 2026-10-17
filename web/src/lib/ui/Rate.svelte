<script lang="ts">
  import { rate } from '$lib/cards.svelte';
  import type { Card } from '$lib/types';
  import Icon from './Icon.svelte';

  let { card, size = 'sm' }: { card: Pick<Card, 'id' | 'myRating'>; size?: 'sm' | 'md' } = $props();

  const OPTIONS = [
    { v: -1, icon: 'thumbdown', label: 'Не для меня' },
    { v: 1, icon: 'thumbup', label: 'Нравится' },
    { v: 2, icon: 'heart', label: 'Обожаю' },
  ] as const;
</script>

<span class="rate {size}" role="group" aria-label="Оценка">
  {#each OPTIONS as o (o.v)}
    <button
      class:on={card.myRating === o.v}
      class:love={o.v === 2}
      aria-pressed={card.myRating === o.v}
      title={o.label}
      aria-label={o.label}
      onclick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        rate(card, o.v);
      }}
    >
      <Icon name={o.icon} size={size === 'md' ? 18 : 15} />
    </button>
  {/each}
</span>

<style>
  .rate {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.35);
    background: rgba(0, 0, 0, 0.55);
  }
  button {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 50%;
    background: none;
    color: #fff;
    cursor: pointer;
    transition:
      background 0.15s,
      transform 0.15s;
  }
  .md button {
    width: 36px;
    height: 36px;
  }
  button:hover {
    background: rgba(255, 255, 255, 0.15);
    transform: scale(1.1);
  }
  button.on {
    background: #fff;
    color: #0a0a0a;
  }
  button.on.love {
    background: var(--accent);
    color: #fff;
  }
</style>
