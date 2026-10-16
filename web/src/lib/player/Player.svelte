<script lang="ts">
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import { clock } from '$lib/fmt';
  import type { PlayInfo } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import { cueAt, parseVtt, type Cue } from './vtt';

  let { fileId }: { fileId: number } = $props();

  let info = $state<PlayInfo | null>(null);
  let video: HTMLVideoElement | undefined = $state();
  let box: HTMLDivElement | undefined = $state();
  let error = $state('');

  // Источник: файл как есть или перепаковка с `offset` (у потока ffmpeg время начинается с 0).
  let mode = $state<'raw' | 'remux'>('raw');
  let offset = $state(0);
  let src = $state('');
  let audio = $state(0);
  let quality = $state(1080);

  let paused = $state(true);
  let current = $state(0);
  let buffered = $state(0);
  let waiting = $state(true);
  let volume = $state(1);
  let muted = $state(false);
  let full = $state(false);
  let shown = $state(true);
  let panel = $state<'' | 'audio' | 'subs' | 'quality'>('');
  let hover = $state<number | null>(null);
  let countdown = $state<number | null>(null);

  let subId = $state('');
  let cues = $state<Cue[]>([]);

  const hevcOk = typeof document !== 'undefined' && document.createElement('video').canPlayType('video/mp4; codecs="hvc1"') !== '';
  const duration = $derived(info?.play.duration || (video && isFinite(video.duration) ? offset + video.duration : 0));
  const time = $derived(offset + current);
  const subtitle = $derived(subId ? cueAt(cues, time) : '');

  function remuxUrl(t: number) {
    const q = new URLSearchParams({ t: t.toFixed(2), audio: String(audio) });
    if (hevcOk) q.set('hevc', '1');
    if (info?.play.transcode) q.set('q', String(quality));
    return `/api/files/${fileId}/play?${q}`;
  }

  function load(at: number) {
    if (!info) return;
    const p = info.play;
    const direct = p.direct && audio === 0 && (!p.hevc || hevcOk);
    if (direct || !p.remux) {
      mode = 'raw';
      offset = 0;
      src = `/api/files/${fileId}/raw`;
      pendingSeek = at;
    } else {
      mode = 'remux';
      offset = at;
      src = remuxUrl(at);
      pendingSeek = 0;
    }
    waiting = true;
  }

  let pendingSeek = 0;

  $effect(() => {
    const id = fileId;
    info = null;
    error = '';
    countdown = null;
    subId = '';
    cues = [];
    api<PlayInfo>(`/api/files/${id}/info`).then(
      (i) => {
        info = i;
        audio = Math.max(0, i.play.audio.findIndex((a) => a.default));
        const forced = i.play.subs.find((s) => s.forced && s.lang?.startsWith('ru'));
        if (forced) pickSubs(forced.id);
        const start = i.position > 5 && (!i.play.duration || i.position < i.play.duration - 20) ? i.position : 0;
        load(start);
      },
      () => (error = 'Не удалось открыть файл'),
    );
  });

  function onLoaded() {
    if (!video) return;
    try {
      video.volume = volume;
    } catch {
      /* iOS не даёт менять громкость */
    }
    if (pendingSeek > 0) video.currentTime = pendingSeek;
    pendingSeek = 0;
    video.play().catch(() => (paused = true));
  }

  function onError() {
    if (!info || !video?.error) return;
    // Браузер не смог сам — пробуем через ffmpeg, если он есть у хоста.
    if (mode === 'raw' && info.play.remux) {
      info.play.direct = false;
      load(time);
      return;
    }
    error = info.play.remux
      ? 'Видео не воспроизводится. Попробуйте ещё раз или скачайте файл'
      : 'Этот формат браузер не играет. Скачайте файл или попросите хоста установить ffmpeg';
  }

  function seek(t: number) {
    if (!video || !duration) return;
    t = Math.max(0, Math.min(duration - 1, t));
    if (mode === 'raw') {
      video.currentTime = t;
    } else {
      const local = t - offset;
      // Внутри уже полученного куска — обычная перемотка, иначе новый поток с нужного места.
      if (local >= 0 && local <= buffered) video.currentTime = local;
      else {
        offset = t;
        current = 0;
        src = remuxUrl(t);
        waiting = true;
      }
    }
    poke();
  }

  function toggle() {
    if (!video) return;
    if (video.paused) video.play().catch(() => {});
    else video.pause();
    poke();
  }

  function setAudio(i: number) {
    panel = '';
    if (i === audio) return;
    audio = i;
    load(time);
  }

  function setQuality(q: number) {
    panel = '';
    quality = q;
    load(time);
  }

  async function pickSubs(id: string) {
    panel = '';
    subId = id;
    cues = [];
    if (!id) return;
    try {
      const r = await fetch(`/api/files/${fileId}/subs/${id}`, { credentials: 'same-origin' });
      if (r.ok) cues = parseVtt(await r.text());
    } catch {
      subId = '';
    }
  }

  // ---------- прогресс ----------
  let lastSaved = 0;
  function save(force = false) {
    if (!duration || time < 1) return;
    const now = Date.now();
    if (!force && now - lastSaved < 10_000) return;
    lastSaved = now;
    fetch(`/api/progress/${fileId}`, {
      method: 'PUT',
      keepalive: true,
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-fsfy': '1' },
      body: JSON.stringify({ position: time, duration }),
    }).catch(() => {});
  }

  $effect(() => {
    const onHide = () => save(true);
    window.addEventListener('pagehide', onHide);
    document.addEventListener('visibilitychange', onHide);
    return () => {
      save(true);
      window.removeEventListener('pagehide', onHide);
      document.removeEventListener('visibilitychange', onHide);
    };
  });

  function onTime() {
    if (!video) return;
    current = video.currentTime;
    if (video.buffered.length) buffered = video.buffered.end(video.buffered.length - 1);
    save();
  }

  function onEnded() {
    save(true);
    if (info?.next) {
      countdown = 8;
      const t = setInterval(() => {
        if (countdown === null) return clearInterval(t);
        countdown--;
        if (countdown <= 0) {
          clearInterval(t);
          goNext();
        }
      }, 1000);
    }
  }

  function goNext() {
    countdown = null;
    if (info?.next) goto(`/watch/${info.next}`, { replaceState: true });
  }

  function back() {
    save(true);
    if (history.length > 1) history.back();
    else goto(info ? `/title/${info.titleId}` : '/');
  }

  // ---------- интерфейс ----------
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  function poke() {
    shown = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (!paused && !panel) shown = false;
    }, 2800);
  }

  function fullscreen() {
    if (document.fullscreenElement) document.exitFullscreen();
    else if (box?.requestFullscreen) box.requestFullscreen().catch(() => {});
    else (video as HTMLVideoElement & { webkitEnterFullscreen?: () => void })?.webkitEnterFullscreen?.();
  }

  function setVolume(v: number) {
    volume = Math.max(0, Math.min(1, v));
    muted = volume === 0;
    if (video) {
      video.volume = volume;
      video.muted = muted;
    }
    try {
      localStorage.setItem('fsfy-volume', String(volume));
    } catch {
      /* приватный режим */
    }
  }

  $effect(() => {
    try {
      const v = Number(localStorage.getItem('fsfy-volume'));
      if (v > 0 && v <= 1) volume = v;
    } catch {
      /* нет хранилища */
    }
    const fs = () => (full = !!document.fullscreenElement);
    document.addEventListener('fullscreenchange', fs);
    return () => document.removeEventListener('fullscreenchange', fs);
  });

  function key(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.metaKey || e.ctrlKey || e.altKey) return;
    const k = e.code;
    if (k === 'Space' || k === 'KeyK') toggle();
    else if (k === 'ArrowRight' || k === 'KeyL') seek(time + 10);
    else if (k === 'ArrowLeft' || k === 'KeyJ') seek(time - 10);
    else if (k === 'ArrowUp') setVolume(volume + 0.1);
    else if (k === 'ArrowDown') setVolume(volume - 0.1);
    else if (k === 'KeyF') fullscreen();
    else if (k === 'KeyM') setVolume(muted ? volume || 1 : 0);
    else if (k === 'KeyN' && info?.next) goNext();
    else if (k === 'KeyC' && info?.play.subs.length) pickSubs(subId ? '' : info.play.subs[0].id);
    else if (k === 'Escape' && panel) panel = '';
    else return;
    e.preventDefault();
  }

  function barPos(e: PointerEvent, el: HTMLElement) {
    const r = el.getBoundingClientRect();
    return Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)) * duration;
  }

  let dragging = $state(false);
  function barDown(e: PointerEvent) {
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    dragging = true;
    hover = barPos(e, el);
  }
  function barMove(e: PointerEvent) {
    hover = barPos(e, e.currentTarget as HTMLElement);
  }
  function barUp(e: PointerEvent) {
    if (dragging) seek(barPos(e, e.currentTarget as HTMLElement));
    dragging = false;
  }

  const bufferedPct = $derived(duration ? Math.min(100, ((offset + buffered) / duration) * 100) : 0);
  const playedPct = $derived(duration ? Math.min(100, (time / duration) * 100) : 0);
</script>

<svelte:window onkeydown={key} />

<div
  class="player"
  class:idle={!shown}
  bind:this={box}
  onpointermove={poke}
  role="application"
  aria-label="Видеоплеер"
>
  {#if src}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video
      bind:this={video}
      {src}
      playsinline
      preload="auto"
      onloadedmetadata={onLoaded}
      ontimeupdate={onTime}
      onprogress={onTime}
      onplay={() => {
        paused = false;
        poke();
      }}
      onpause={() => {
        paused = true;
        shown = true;
        save(true);
      }}
      onwaiting={() => (waiting = true)}
      onplaying={() => (waiting = false)}
      oncanplay={() => (waiting = false)}
      onended={onEnded}
      onerror={onError}
      onclick={toggle}
      ondblclick={fullscreen}
    ></video>
  {/if}

  {#if subtitle}
    <div class="subs" class:up={shown}>{subtitle}</div>
  {/if}

  {#if waiting && !error}
    <div class="spinner" aria-label="Загрузка"></div>
  {/if}

  {#if error}
    <div class="err">
      <Icon name="info" size={40} />
      <p>{error}</p>
      <div class="row">
        <button class="btn" onclick={back}><Icon name="back" size={16} /> Назад</button>
        <a class="btn white" href="/api/files/{fileId}/download" download><Icon name="download" size={16} /> Скачать</a>
      </div>
    </div>
  {/if}

  {#if countdown !== null && info?.next}
    <div class="nextup">
      <span>Следующая серия через {countdown}</span>
      <button class="btn white sm" onclick={goNext}><Icon name="next" size={14} /> Смотреть сейчас</button>
      <button class="btn sm" onclick={() => (countdown = null)}>Отмена</button>
    </div>
  {/if}

  <div class="top">
    <button class="btn ghost icon" aria-label="Назад" onclick={back}><Icon name="back" size={24} /></button>
    {#if info}
      <div class="name">
        <b>{info.title}</b>
        {#if info.episode}<span>{info.episode}</span>{/if}
      </div>
    {/if}
  </div>

  {#if paused && !waiting && !error && src}
    <button class="center" aria-label="Смотреть" onclick={toggle}><Icon name="play" size={36} /></button>
  {/if}

  <div class="bottom">
    <div
      class="bar"
      role="slider"
      tabindex="-1"
      aria-label="Перемотка"
      aria-valuemin={0}
      aria-valuemax={Math.round(duration)}
      aria-valuenow={Math.round(time)}
      onpointerdown={barDown}
      onpointermove={barMove}
      onpointerup={barUp}
      onpointerleave={() => !dragging && (hover = null)}
    >
      <div class="rail">
        <i class="buf" style:width="{bufferedPct}%"></i>
        <i class="played" style:width="{dragging && hover !== null ? (hover / duration) * 100 : playedPct}%"></i>
      </div>
      {#if hover !== null && duration}
        <span class="tip" style:left="{(hover / duration) * 100}%">{clock(hover)}</span>
      {/if}
    </div>
    <div class="controls">
      <button class="cbtn" aria-label={paused ? 'Смотреть' : 'Пауза'} onclick={toggle}><Icon name={paused ? 'play' : 'pause'} size={24} /></button>
      <button class="cbtn" aria-label="Назад на 10 секунд" onclick={() => seek(time - 10)}><Icon name="rewind" size={22} /></button>
      <button class="cbtn" aria-label="Вперёд на 10 секунд" onclick={() => seek(time + 10)}><Icon name="forward" size={22} /></button>
      <div class="vol">
        <button class="cbtn" aria-label={muted ? 'Включить звук' : 'Выключить звук'} onclick={() => setVolume(muted ? volume || 1 : 0)}>
          <Icon name={muted || volume === 0 ? 'mute' : 'volume'} size={22} />
        </button>
        <input
          type="range"
          min="0"
          max="1"
          step="0.02"
          value={muted ? 0 : volume}
          oninput={(e) => setVolume(Number((e.target as HTMLInputElement).value))}
          aria-label="Громкость"
        />
      </div>
      <span class="time">{clock(time)} <span class="faint">/ {clock(duration)}</span></span>
      <span class="grow"></span>
      {#if info && info.play.audio.length > 1}
        <button class="cbtn" class:on={panel === 'audio'} aria-label="Озвучка" title="Озвучка" onclick={() => (panel = panel === 'audio' ? '' : 'audio')}>
          <Icon name="audio" size={22} />
        </button>
      {/if}
      {#if info?.play.subs.length}
        <button class="cbtn" class:on={panel === 'subs' || !!subId} aria-label="Субтитры" title="Субтитры" onclick={() => (panel = panel === 'subs' ? '' : 'subs')}>
          <Icon name="captions" size={22} />
        </button>
      {/if}
      {#if info?.play.transcode && mode === 'remux'}
        <button class="cbtn q" class:on={panel === 'quality'} aria-label="Качество" title="Качество" onclick={() => (panel = panel === 'quality' ? '' : 'quality')}>
          {quality}p
        </button>
      {/if}
      {#if info?.next}
        <button class="cbtn" aria-label="Следующая серия" title="Следующая серия" onclick={goNext}><Icon name="next" size={22} /></button>
      {/if}
      <button class="cbtn" aria-label="Во весь экран" onclick={fullscreen}><Icon name={full ? 'exitfs' : 'fullscreen'} size={22} /></button>
    </div>
  </div>

  {#if panel && info}
    <div class="panel" role="menu">
      {#if panel === 'audio'}
        <h4>Озвучка</h4>
        {#each info.play.audio as a (a.index)}
          <button role="menuitemradio" aria-checked={audio === a.index} class:on={audio === a.index} onclick={() => setAudio(a.index)}>
            <Icon name="check" size={16} /> {a.label}
          </button>
        {/each}
      {:else if panel === 'subs'}
        <h4>Субтитры</h4>
        <button role="menuitemradio" aria-checked={!subId} class:on={!subId} onclick={() => pickSubs('')}><Icon name="check" size={16} /> Выключены</button>
        {#each info.play.subs as s (s.id)}
          <button role="menuitemradio" aria-checked={subId === s.id} class:on={subId === s.id} onclick={() => pickSubs(s.id)}>
            <Icon name="check" size={16} /> {s.label}
          </button>
        {/each}
      {:else}
        <h4>Качество</h4>
        {#each [1080, 720, 480] as q (q)}
          <button role="menuitemradio" aria-checked={quality === q} class:on={quality === q} onclick={() => setQuality(q)}>
            <Icon name="check" size={16} /> {q}p{q === 480 ? ' · медленный интернет' : ''}
          </button>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .player {
    position: fixed;
    inset: 0;
    background: #000;
    color: #fff;
    overflow: hidden;
    user-select: none;
  }
  .player.idle {
    cursor: none;
  }
  video {
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: #000;
  }
  .top,
  .bottom {
    position: absolute;
    left: 0;
    right: 0;
    padding: 18px clamp(14px, 3vw, 36px);
    transition:
      opacity 0.3s,
      transform 0.3s;
  }
  .top {
    top: 0;
    display: flex;
    align-items: center;
    gap: 14px;
    padding-top: calc(18px + env(safe-area-inset-top));
    background: linear-gradient(180deg, rgba(0, 0, 0, 0.75), transparent);
  }
  .bottom {
    bottom: 0;
    padding-bottom: calc(14px + env(safe-area-inset-bottom));
    background: linear-gradient(0deg, rgba(0, 0, 0, 0.85), transparent);
  }
  .idle .top {
    opacity: 0;
    transform: translateY(-10px);
    pointer-events: none;
  }
  .idle .bottom {
    opacity: 0;
    transform: translateY(10px);
    pointer-events: none;
  }
  .name {
    display: grid;
    line-height: 1.25;
    min-width: 0;
  }
  .name b {
    font-size: 17px;
  }
  .name span {
    font-size: 13px;
    color: #cfcce4;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar {
    position: relative;
    height: 22px;
    display: flex;
    align-items: center;
    cursor: pointer;
    touch-action: none;
  }
  .rail {
    position: relative;
    width: 100%;
    height: 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.22);
    transition: height 0.15s;
  }
  .bar:hover .rail {
    height: 6px;
  }
  .rail i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 4px;
  }
  .buf {
    background: rgba(255, 255, 255, 0.35);
  }
  .played {
    background: var(--accent-grad);
  }
  .played::after {
    content: '';
    position: absolute;
    right: -7px;
    top: 50%;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #fff;
    transform: translateY(-50%) scale(0);
    transition: transform 0.15s;
  }
  .bar:hover .played::after {
    transform: translateY(-50%) scale(1);
  }
  .tip {
    position: absolute;
    bottom: 24px;
    transform: translateX(-50%);
    padding: 3px 8px;
    border-radius: 6px;
    background: rgba(20, 17, 43, 0.9);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 6px;
  }
  .cbtn {
    height: 44px;
    min-width: 44px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: #fff;
    cursor: pointer;
    transition: background 0.15s;
  }
  .cbtn:hover,
  .cbtn.on {
    background: rgba(255, 255, 255, 0.12);
  }
  .cbtn.q {
    font-size: 13px;
    font-weight: 700;
    padding: 0 10px;
  }
  .vol {
    display: flex;
    align-items: center;
  }
  .vol input {
    width: 0;
    opacity: 0;
    accent-color: #fff;
    transition:
      width 0.2s,
      opacity 0.2s;
  }
  .vol:hover input,
  .vol input:focus {
    width: 90px;
    opacity: 1;
  }
  .time {
    margin-left: 10px;
    font-size: 14px;
    font-variant-numeric: tabular-nums;
  }
  .grow {
    flex: 1;
  }
  .center {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 86px;
    height: 86px;
    border-radius: 50%;
    border: 0;
    display: grid;
    place-items: center;
    padding-left: 6px;
    background: #fff;
    color: var(--accent);
    cursor: pointer;
    box-shadow: 0 0 0 10px rgba(255, 255, 255, 0.15);
  }
  .spinner {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 56px;
    height: 56px;
    margin: -28px;
    border-radius: 50%;
    border: 4px solid rgba(255, 255, 255, 0.15);
    border-top-color: #fff;
    animation: spin 0.8s linear infinite;
    pointer-events: none;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .subs {
    position: absolute;
    left: 50%;
    bottom: 7%;
    transform: translateX(-50%);
    max-width: 86%;
    padding: 4px 12px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.55);
    font-size: clamp(16px, 2.6vw, 30px);
    line-height: 1.3;
    text-align: center;
    white-space: pre-line;
    text-shadow: 0 1px 3px #000;
    transition: bottom 0.3s;
    pointer-events: none;
  }
  .subs.up {
    bottom: calc(110px + env(safe-area-inset-bottom));
  }
  .panel {
    position: absolute;
    right: clamp(14px, 3vw, 36px);
    bottom: calc(96px + env(safe-area-inset-bottom));
    min-width: 260px;
    max-height: 60vh;
    overflow-y: auto;
    padding: 8px;
    border-radius: 14px;
    background: rgba(20, 17, 43, 0.94);
    backdrop-filter: blur(14px);
    border: 1px solid var(--line);
  }
  .panel h4 {
    margin: 6px 10px 8px;
    font-size: 13px;
    color: var(--muted);
    font-weight: 600;
  }
  .panel button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: #fff;
    text-align: left;
    cursor: pointer;
    font-size: 14px;
  }
  .panel button :global(svg) {
    opacity: 0;
  }
  .panel button.on :global(svg) {
    opacity: 1;
    color: var(--accent);
  }
  .panel button:hover {
    background: rgba(255, 255, 255, 0.08);
  }
  .err {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
    padding: 24px;
    text-align: center;
    background: rgba(0, 0, 0, 0.7);
  }
  .err p {
    max-width: 420px;
    margin: 0;
  }
  .err .row {
    display: flex;
    gap: 10px;
  }
  .nextup {
    position: absolute;
    right: clamp(14px, 3vw, 36px);
    bottom: calc(110px + env(safe-area-inset-bottom));
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px 10px 16px;
    border-radius: 14px;
    background: rgba(20, 17, 43, 0.94);
    border: 1px solid var(--line);
    font-size: 14px;
  }
  @media (max-width: 640px) {
    .vol input,
    .vol:hover input {
      display: none;
    }
    .time {
      font-size: 12px;
      margin-left: 4px;
    }
    .controls .cbtn:nth-child(2),
    .controls .cbtn:nth-child(3) {
      display: none;
    }
  }
</style>
