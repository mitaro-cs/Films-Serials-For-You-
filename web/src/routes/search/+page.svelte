<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api } from '$lib/api';
  import type { Card } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import Ticket from '$lib/ui/Ticket.svelte';

  const q = $derived(page.url.searchParams.get('q') ?? '');
  let items = $state<Card[] | null>(null);
  let text = $state('');

  $effect(() => {
    text = q;
  });

  $effect(() => {
    const query = q.trim();
    if (!query) {
      items = null;
      return;
    }
    const t = setTimeout(() => {
      api<Card[]>(`/api/titles?sort=rating&q=${encodeURIComponent(query)}`).then((r) => (items = r), () => (items = []));
    }, 180);
    return () => clearTimeout(t);
  });
</script>

<div class="page">
  <form class="big" role="search" onsubmit={(e) => e.preventDefault()}>
    <Icon name="search" size={22} />
    <!-- svelte-ignore a11y_autofocus -->
    <input
      type="search"
      placeholder="Название фильма или сериала"
      bind:value={text}
      oninput={() => goto(`/search?q=${encodeURIComponent(text)}`, { replaceState: true, keepFocus: true, noScroll: true })}
      autofocus
      aria-label="Поиск"
    />
  </form>
  {#if items?.length}
    <div class="grid">
      {#each items as card (card.id)}<Ticket {card} />{/each}
    </div>
  {:else if items}
    <div class="empty"><Icon name="search" size={40} /> По запросу «{q}» ничего нет</div>
  {/if}
</div>

<style>
  .big {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 56px;
    padding: 0 18px;
    margin-bottom: 28px;
    border-radius: 16px;
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .big:focus-within {
    border-color: var(--accent-2);
  }
  .big input {
    flex: 1;
    border: 0;
    outline: none;
    background: none;
    font-size: 18px;
    color: var(--text);
  }
</style>
