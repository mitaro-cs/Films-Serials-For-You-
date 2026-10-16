<script lang="ts">
  import Icon from './Icon.svelte';
  let { rating, size = 12 }: { rating: number | null; size?: number } = $props();
  const filled = $derived(Math.round((rating ?? 0) / 2));
</script>

{#if rating}
  <span class="stars" aria-label="Рейтинг {rating.toFixed(1)} из 10">
    {#each [1, 2, 3, 4, 5] as i (i)}
      <span class:on={i <= filled}><Icon name="star" {size} /></span>
    {/each}
  </span>
{/if}

<style>
  .stars {
    display: inline-flex;
    gap: 1px;
    color: rgba(255, 255, 255, 0.18);
  }
  .on {
    color: var(--imdb);
  }
</style>
