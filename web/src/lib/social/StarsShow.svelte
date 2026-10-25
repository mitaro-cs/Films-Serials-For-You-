<script lang="ts">
  // Звёзды только для показа (отзыв): value 1..10 - половинки.
  let { value, size = 13 }: { value: number; size?: number } = $props();
  const STAR = 'M12 2.8l2.8 5.9 6.4.8-4.7 4.4 1.2 6.4L12 17.2l-5.7 3.1 1.2-6.4-4.7-4.4 6.4-.8z';
</script>

<span class="s" role="img" aria-label="{String(value / 2).replace('.', ',')} из 5">
  {#each [1, 2, 3, 4, 5] as i (i)}
    <svg width={size} height={size} viewBox="0 0 24 24" style:--fill={value >= i * 2 ? '100%' : value === i * 2 - 1 ? '50%' : '0%'}>
      <path d={STAR} class="bg" />
      <path d={STAR} class="fg" />
    </svg>
  {/each}
</span>

<style>
  .s {
    display: inline-flex;
    gap: 1px;
  }
  .bg {
    fill: rgba(255, 255, 255, 0.18);
  }
  .fg {
    fill: currentColor;
    clip-path: inset(0 calc(100% - var(--fill)) 0 0);
  }
</style>
