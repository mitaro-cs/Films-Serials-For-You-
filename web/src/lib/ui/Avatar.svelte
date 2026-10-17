<script lang="ts">
  // Аватар: своё фото (круг) или рисованное лицо (12 цветов и выражений).
  let {
    avatar = 0,
    size = 40,
    kids = false,
    photo = null,
  }: { avatar?: number; size?: number; kids?: boolean; photo?: string | null } = $props();

  const PALETTE: [string, string][] = [
    ['#ff4f7b', '#b8205a'],
    ['#6d5cff', '#3a2bb8'],
    ['#1fb6ff', '#0b6fb8'],
    ['#ffb84d', '#e0702a'],
    ['#3ddc97', '#159a63'],
    ['#c04dff', '#7a1fb8'],
    ['#ff6b3d', '#b8331f'],
    ['#2bd6d6', '#138a9a'],
    ['#f5c518', '#c48a0b'],
    ['#ff7ad9', '#b83a8c'],
    ['#8f9bff', '#4c55b8'],
    ['#a3e635', '#5f9a13'],
  ];
  const [a, b] = $derived(PALETTE[((avatar % 12) + 12) % 12]);
  const id = $props.id();
  // Разные лица: глаза и улыбка чуть отличаются по номеру.
  const eyes = $derived(avatar % 3);
  const smile = $derived(avatar % 4);
</script>

{#if photo}
  <img src={photo} alt="" width={size} height={size} class="ava photo" loading="lazy" decoding="async" />
{:else}
<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true" class="ava">
  <defs>
    <linearGradient id="g{id}" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color={a} />
      <stop offset="1" stop-color={b} />
    </linearGradient>
  </defs>
  <circle cx="32" cy="32" r="32" fill="url(#g{id})" />
  {#if eyes === 0}
    <rect x="18" y="20" width="7" height="12" rx="3.5" fill="#fff" />
    <rect x="39" y="20" width="7" height="12" rx="3.5" fill="#fff" />
  {:else if eyes === 1}
    <circle cx="22" cy="26" r="4.5" fill="#fff" />
    <circle cx="42" cy="26" r="4.5" fill="#fff" />
  {:else}
    <path d="M16 27q5-7 10 0M38 27q5-7 10 0" stroke="#fff" stroke-width="4" fill="none" stroke-linecap="round" />
  {/if}
  {#if smile === 0}
    <path d="M18 40q14 12 28 0" stroke="#fff" stroke-width="4.5" fill="none" stroke-linecap="round" />
  {:else if smile === 1}
    <path d="M20 40q12 14 24 0z" fill="#fff" />
  {:else if smile === 2}
    <path d="M22 42q10 6 20 0" stroke="#fff" stroke-width="4.5" fill="none" stroke-linecap="round" />
  {:else}
    <path d="M18 38q14 16 28 0" stroke="#fff" stroke-width="4.5" fill="none" stroke-linecap="round" />
  {/if}
  {#if kids}
    <circle cx="50" cy="14" r="8" fill="#fff" />
    <path d="M50 9l1.5 3.2 3.5.4-2.6 2.4.7 3.4-3.1-1.8-3.1 1.8.7-3.4-2.6-2.4 3.5-.4z" fill={b} />
  {/if}
</svg>
{/if}

<style>
  .ava {
    display: block;
    flex: none;
    border-radius: 50%;
  }
  .photo {
    object-fit: cover;
    background: var(--surface-2);
  }
</style>
