<script lang="ts">
  import type { ContinueItem } from '$lib/types';
  import { placeholder } from './art';
  import Icon from './Icon.svelte';

  let { item, onremove }: { item: ContinueItem; onremove?: () => void } = $props();
  const img = $derived(item.kind === 'series' ? (item.still ?? item.backdrop) : (item.backdrop ?? item.still));
  const pct = $derived(item.next.duration ? Math.min(100, (item.next.position / item.next.duration) * 100) : 0);
</script>

<div class="wide">
  <a href="/watch/{item.next.fileId}" class="frame" style:background={img ? undefined : placeholder(item.title)}>
    {#if img}<img src={img} alt="" loading="lazy" />{/if}
    <span class="play"><Icon name="play" size={22} /></span>
    {#if pct > 0}<span class="bar"><i style:width="{pct}%"></i></span>{/if}
  </a>
  <div class="text">
    <a href="/title/{item.id}" class="t">{item.title}</a>
    {#if onremove}
      <button class="x" aria-label="Убрать из «Продолжить»" title="Убрать" onclick={onremove}><Icon name="close" size={16} /></button>
    {/if}
  </div>
  <div class="sub">{item.next.label || (pct > 0 ? 'Продолжить' : 'Начать')}</div>
</div>

<style>
  .frame {
    position: relative;
    display: block;
    aspect-ratio: 16 / 9;
    border-radius: 10px;
    overflow: hidden;
    background: var(--surface);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.4s;
  }
  .frame:hover img {
    transform: scale(1.05);
  }
  .play {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 50px;
    height: 50px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.55);
    border: 1.5px solid rgba(255, 255, 255, 0.85);
    backdrop-filter: blur(4px);
    padding-left: 3px;
    opacity: 0.9;
    transition: transform 0.2s;
  }
  .frame:hover .play {
    transform: scale(1.1);
  }
  .bar {
    position: absolute;
    inset: auto 10px 10px;
    height: 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.25);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .text {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    margin-top: 8px;
  }
  .t {
    font-weight: 600;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .x {
    border: 0;
    background: none;
    color: var(--faint);
    cursor: pointer;
    padding: 2px;
  }
  .x:hover {
    color: var(--text);
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
