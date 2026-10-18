<script lang="ts">
  import { clock } from '$lib/fmt';
  import type { VideoLink } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';

  // Видео по ссылке: кадр 16:9, название, сайт. Открывается в нашем плеере.
  let { link, onremove }: { link: VideoLink; onremove?: () => void } = $props();
  const SITE = { rutube: 'Rutube', vk: 'VK Видео', youtube: 'YouTube', direct: 'Файл' } as const;
</script>

<div class="lc">
  <a class="art" href="/watch/link/{link.id}" aria-label="Смотреть: {link.label}">
    {#if link.thumb}<img src={link.thumb} alt="" loading="lazy" decoding="async" />{/if}
    <span class="play"><Icon name="play" size={18} /></span>
    <span class="site">{SITE[link.kind]}</span>
    {#if link.duration}<span class="dur">{clock(link.duration)}</span>{/if}
  </a>
  <div class="foot">
    <span class="name" title={link.label}>{link.label}</span>
    {#if onremove}<button class="x" aria-label="Убрать ссылку" title="Убрать ссылку" onclick={onremove}><Icon name="close" size={14} /></button>{/if}
  </div>
</div>

<style>
  .lc {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .art {
    position: relative;
    display: block;
    aspect-ratio: 16 / 9;
    border-radius: 12px;
    overflow: hidden;
    background: var(--surface);
  }
  .art img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .play {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.92);
    color: #000;
    transition: transform 0.2s;
  }
  .art:hover .play {
    transform: scale(1.1);
  }
  .site,
  .dur {
    position: absolute;
    bottom: 8px;
    padding: 1px 7px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.72);
    font-size: 11px;
    font-weight: 700;
  }
  .site {
    left: 8px;
  }
  .dur {
    right: 8px;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .x {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    flex: none;
    border: 0;
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--muted);
    cursor: pointer;
  }
</style>
