<script lang="ts">
  import { api } from '$lib/api';
  import type { Card } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import Poster from '$lib/ui/Poster.svelte';

  let items = $state<Card[] | null>(null);
  $effect(() => {
    api<Card[]>('/api/list').then((r) => (items = r), () => (items = []));
  });
</script>

<div class="page">
  <h1 class="section-title" style="font-size: 30px"><Icon name="bookmark" size={26} /> Мой список</h1>
  {#if items?.length}
    <div class="grid">
      {#each items as card (card.id)}<Poster {card} />{/each}
    </div>
  {:else if items}
    <div class="empty">
      <Icon name="bookmark" size={40} />
      Добавляйте фильмы кнопкой «Смотреть позже» — они соберутся здесь.
      <a class="btn" href="/movies">К фильмам</a>
    </div>
  {/if}
</div>
