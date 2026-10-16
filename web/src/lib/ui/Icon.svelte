<script lang="ts">
  import { FILLED, ICONS, type IconName } from './icons';

  let { name, size = 20, stroke = 2 }: { name: IconName; size?: number; stroke?: number } = $props();
  const filled = $derived(FILLED.has(name));
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={filled ? 'currentColor' : 'none'}
  stroke={filled ? 'none' : 'currentColor'}
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={ICONS[name]} />
</svg>

<style>
  svg {
    flex: none;
    display: block;
  }
</style>
