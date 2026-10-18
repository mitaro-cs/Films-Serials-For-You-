<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, errorText } from '$lib/api';
  import type { VideoLink } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';

  // Видео по ссылке на весь экран: Rutube, VK и YouTube — их встроенный плеер, прямой файл
  // (.mp4, .webm, .m3u8) — наш <video>; hls.js — только для .m3u8 вне Safari и только лениво.
  const id = $derived(Number(page.params.id));
  let link = $state<VideoLink | null>(null);
  let error = $state('');
  let video = $state<HTMLVideoElement>();

  $effect(() => {
    link = null;
    error = '';
    api<VideoLink>(`/api/links/${id}`).then(
      (l) => (link = l),
      (e) => (error = errorText(e)),
    );
  });

  $effect(() => {
    const v = video;
    const l = link;
    if (!v || !l || l.kind !== 'direct') return;
    let hls: { destroy(): void } | null = null;
    let gone = false;
    if (/\.m3u8(\?|$)/i.test(l.url) && !v.canPlayType('application/vnd.apple.mpegurl')) {
      import('hls.js/light').then(({ default: Hls }) => {
        if (gone) return;
        if (!Hls.isSupported()) return void (error = 'Этот браузер не умеет такие трансляции');
        const h = new Hls();
        h.loadSource(l.url);
        h.attachMedia(v);
        hls = h;
      });
    } else {
      v.src = l.url;
    }
    return () => {
      gone = true;
      hls?.destroy();
    };
  });

  function back() {
    if (history.length > 1) history.back();
    else goto('/discover');
  }

  // Esc — назад в сервис (фокус внутри чужого плеера Esc сюда не отдаёт — для этого кнопка).
  $effect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || document.fullscreenElement) return;
      e.preventDefault();
      back();
    };
    addEventListener('keydown', key);
    return () => removeEventListener('keydown', key);
  });
</script>

<div class="stage">
  {#if link?.embed}
    <iframe
      src={link.embed}
      title={link.label}
      allow="autoplay; fullscreen; picture-in-picture; encrypted-media; clipboard-write"
      allowfullscreen
      referrerpolicy="strict-origin-when-cross-origin"
    ></iframe>
  {:else if link}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video bind:this={video} controls autoplay playsinline onerror={() => (error = 'Видео не открывается — ссылка устарела или сайт её не отдаёт')}></video>
  {/if}
  {#if error}
    <div class="msg"><Icon name="offline" size={36} /><p>{error}</p><button class="btn" onclick={back}>Назад</button></div>
  {:else if !link}
    <div class="msg"><span class="spin" aria-label="Загрузка"></span></div>
  {/if}
  <div class="bar">
    <button class="round" onclick={back} aria-label="Назад" title="Назад (Esc)"><Icon name="back" size={22} /></button>
    {#if link}<span class="name">{link.label}</span>{/if}
  </div>
</div>

<style>
  .stage {
    position: fixed;
    inset: 0;
    background: #000;
    display: grid;
  }
  iframe,
  video {
    grid-area: 1 / 1;
    width: 100%;
    height: 100%;
    border: 0;
    background: #000;
  }
  .bar {
    position: absolute;
    top: calc(12px + env(safe-area-inset-top));
    left: 12px;
    right: 12px;
    display: flex;
    align-items: center;
    gap: 12px;
    pointer-events: none;
  }
  .round {
    pointer-events: auto;
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    flex: none;
    border: 0;
    border-radius: 50%;
    background: rgba(30, 30, 32, 0.85);
    color: #fff;
    cursor: pointer;
  }
  .round:hover {
    background: rgba(60, 60, 64, 0.95);
  }
  .name {
    min-width: 0;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-shadow: 0 1px 6px rgba(0, 0, 0, 0.9);
  }
  /* Над встроенным плеером название мешает его собственной шапке — показываем только у файла. */
  .stage:has(iframe) .name {
    display: none;
  }
  .msg {
    grid-area: 1 / 1;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
    text-align: center;
    padding: 24px;
  }
  .msg p {
    margin: 0;
  }
  .spin {
    width: 32px;
    height: 32px;
    border: 3px solid rgba(255, 255, 255, 0.2);
    border-top-color: #fff;
    border-radius: 50%;
    animation: turn 0.8s linear infinite;
  }
  @keyframes turn {
    to {
      transform: rotate(1turn);
    }
  }
</style>
