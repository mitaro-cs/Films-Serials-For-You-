<script lang="ts">
  import { characterOf } from './characters';

  // Аватар: своё фото (круг) или персонаж Nova (12 мультяшных героев, номер - по кругу).
  let {
    avatar = 0,
    size = 40,
    kids = false,
    photo = null,
  }: { avatar?: number; size?: number; kids?: boolean; photo?: string | null } = $props();
  const hero = $derived(characterOf(avatar));
  const uid = $props.id();
</script>

{#if photo}
  <img src={photo} alt="" width={size} height={size} class="ava photo" loading="lazy" decoding="async" />
{:else}
<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true" class="ava">
  <defs><clipPath id="c{uid}"><circle cx="32" cy="32" r="32" /></clipPath></defs>
  <circle cx="32" cy="32" r="32" fill={hero.bg} />
  <g clip-path="url(#c{uid})">{@html hero.art}</g>
  {#if kids}
    <circle cx="50" cy="14" r="8" fill="#fff" />
    <path d="M50 9l1.5 3.2 3.5.4-2.6 2.4.7 3.4-3.1-1.8-3.1 1.8.7-3.4-2.6-2.4 3.5-.4z" fill="#f5a524" />
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
