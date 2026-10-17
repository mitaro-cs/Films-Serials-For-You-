<script lang="ts">
  import { goto } from '$app/navigation';
  import { untrack } from 'svelte';
  import { api, put } from '$lib/api';
  import { clock } from '$lib/fmt';
  import { session } from '$lib/session.svelte';
  import type { PlayInfo } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import type HlsType from 'hls.js';
  import { levels, trickFrame } from './levels';
  import { cueAt, parseVtt, type Cue } from './vtt';

  let { fileId }: { fileId: number } = $props();

  let info = $state<PlayInfo | null>(null);
  let video: HTMLVideoElement | undefined = $state();
  let box: HTMLDivElement | undefined = $state();
  let error = $state('');
  let toast = $state('');

  // Источник: файл как есть, перепаковка с места `offset` (время потока ffmpeg с нуля) или
  // адаптивный HLS (как у Netflix: качество под скорость интернета).
  let mode = $state<'raw' | 'remux' | 'hls'>('raw');
  let quality = $state<'orig' | 'auto' | number>('orig');
  let hlsHeight = $state<number | null>(null);
  let offset = $state(0);
  let audio = $state(0);
  let hls: HlsType | null = null;
  let pendingSeek = 0;

  let paused = $state(true);
  let current = $state(0);
  let buffered = $state(0);
  let waiting = $state(true);
  let volume = $state(1);
  let muted = $state(false);
  let full = $state(false);
  let shown = $state(true);
  let panel = $state<'' | 'audio' | 'subs' | 'quality' | 'marks'>('');
  let hover = $state<number | null>(null);
  let barWidth = $state(0);
  let countdown = $state<number | null>(null);
  let creditsSkipped = $state(false);

  let subId = $state('');
  let cues = $state<Cue[]>([]);

  const hevcOk =
    typeof document !== 'undefined' && document.createElement('video').canPlayType('video/mp4; codecs="hvc1"') !== '';
  const nativeHls =
    typeof document !== 'undefined' &&
    document.createElement('video').canPlayType('application/vnd.apple.mpegurl') !== '' &&
    !('MediaSource' in globalThis && /Chrome|Android/.test(navigator.userAgent));

  const duration = $derived(info?.play.duration || (video && isFinite(video.duration) ? offset + video.duration : 0));
  const time = $derived(offset + current);
  const subtitle = $derived(subId ? cueAt(cues, time) : '');
  const intro = $derived(info?.markers.intro ?? null);
  const inIntro = $derived(!!intro && time >= intro[0] && time < intro[1] - 1);
  const credits = $derived(info?.markers.credits ?? null);
  const inCredits = $derived(
    !!info?.next && info.kind === 'series' && credits !== null && time >= credits && !creditsSkipped && !error,
  );
  const origOk = $derived.by(() => {
    const p = info?.play;
    if (!p) return false;
    return (p.direct && audio === 0 && (!p.hevc || hevcOk)) || (p.remux && !p.transcode) || (!p.remux && !info?.hls);
  });
  const ladder = $derived(levels(info?.play.height ?? null));
  const qualityLabel = $derived(
    mode === 'hls' ? (quality === 'auto' ? `Авто${hlsHeight ? ` · ${hlsHeight}p` : ''}` : `${quality}p`) : 'Оригинал',
  );

  function remuxUrl(t: number) {
    const q = new URLSearchParams({ t: t.toFixed(2), audio: String(audio) });
    if (hevcOk) q.set('hevc', '1');
    return `/api/files/${fileId}/play?${q}`;
  }

  function destroyHls() {
    hls?.destroy();
    hls = null;
    hlsHeight = null;
  }

  async function load(at: number) {
    if (!info || !video) return;
    destroyHls();
    error = '';
    const p = info.play;
    if (quality === 'orig' && !origOk && info.hls) quality = 'auto';
    if (quality !== 'orig' && !info.hls) quality = 'orig';
    waiting = true;
    if (quality === 'orig') {
      const direct = (p.direct && audio === 0 && (!p.hevc || hevcOk)) || !p.remux;
      mode = direct ? 'raw' : 'remux';
      offset = direct ? 0 : at;
      pendingSeek = direct ? at : 0;
      video.src = direct ? `/api/files/${fileId}/raw` : remuxUrl(at);
      return;
    }
    mode = 'hls';
    offset = 0;
    const url =
      quality === 'auto'
        ? `/api/files/${fileId}/hls/master.m3u8?audio=${audio}`
        : `/api/files/${fileId}/hls/${quality}/index.m3u8?audio=${audio}`;
    if (nativeHls) {
      pendingSeek = at;
      video.src = url;
      return;
    }
    const { default: Hls } = await import('hls.js/light');
    if (!Hls.isSupported()) {
      error = 'Этот браузер не умеет адаптивное видео — скачайте файл или откройте в другом браузере';
      return;
    }
    let retries = 0;
    const h = new Hls({
      startPosition: at,
      maxBufferLength: 40,
      backBufferLength: 60,
      fragLoadPolicy: {
        default: {
          maxTimeToFirstByteMs: 45_000,
          maxLoadTimeMs: 120_000,
          timeoutRetry: { maxNumRetry: 4, retryDelayMs: 500, maxRetryDelayMs: 4000 },
          errorRetry: { maxNumRetry: 6, retryDelayMs: 1000, maxRetryDelayMs: 8000 },
        },
      },
    });
    hls = h;
    pendingSeek = 0;
    h.on(Hls.Events.LEVEL_SWITCHED, (_e, d) => (hlsHeight = h.levels[d.level]?.height ?? null));
    h.on(Hls.Events.MANIFEST_PARSED, () => video?.play().catch(() => (paused = true)));
    h.on(Hls.Events.ERROR, (_e, d) => {
      if (!d.fatal) return;
      if (retries++ < 3) {
        if (d.type === Hls.ErrorTypes.MEDIA_ERROR) h.recoverMediaError();
        else h.startLoad();
        return;
      }
      error =
        d.response?.code === 503
          ? 'Компьютер хоста сейчас занят другими просмотрами — попробуйте через минуту'
          : 'Видео не загружается. Проверьте интернет или скачайте файл';
    });
    h.loadSource(url);
    h.attachMedia(video);
  }

  $effect(() => {
    const id = fileId;
    info = null;
    error = '';
    countdown = null;
    creditsSkipped = false;
    subId = '';
    cues = [];
    api<PlayInfo>(`/api/files/${id}/info`).then(
      (i) => {
        info = i;
        audio = Math.max(0, i.play.audio.findIndex((a) => a.default));
        quality = 'orig';
        const forced = i.play.subs.find((s) => s.forced && s.lang?.startsWith('ru'));
        if (forced) pickSubs(forced.id);
        const start = i.position > 5 && (!i.play.duration || i.position < i.play.duration - 20) ? i.position : 0;
        load(start);
      },
      () => (error = 'Не удалось открыть файл'),
    );
    return destroyHls;
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
    if (!hls) video.play().catch(() => (paused = true));
  }

  function onError() {
    if (!info || !video?.error || hls) return;
    // Браузер не смог сам — адаптивный поток или перепаковка через ffmpeg у хоста.
    if (mode === 'raw' && (info.hls || info.play.remux)) {
      info.play.direct = false;
      if (info.hls) quality = 'auto';
      load(time);
      return;
    }
    error = info.play.remux
      ? 'Видео не воспроизводится. Попробуйте ещё раз или скачайте файл'
      : 'Этот формат браузер не играет. Скачайте файл или попросите хоста установить ffmpeg';
  }

  // Медленный интернет: три остановки за 45 секунд — включаем автокачество, как Netflix.
  let stalls: number[] = [];
  function onWaiting() {
    waiting = true;
    if (!info?.hls || mode === 'hls' || !video || video.seeking || current < 2) return;
    const now = Date.now();
    stalls = [...stalls.filter((t) => now - t < 45_000), now];
    if (stalls.length >= 3) {
      stalls = [];
      quality = 'auto';
      say('Медленный интернет — включили автокачество');
      load(time);
    }
  }

  function say(text: string) {
    toast = text;
    setTimeout(() => toast === text && (toast = ''), 4000);
  }

  function seek(t: number) {
    if (!video || !duration) return;
    t = Math.max(0, Math.min(duration - 1, t));
    if (mode !== 'remux') {
      video.currentTime = t;
    } else {
      const local = t - offset;
      // Внутри уже полученного куска — обычная перемотка, иначе новый поток с нужного места.
      if (local >= 0 && local <= buffered) video.currentTime = local;
      else {
        offset = t;
        current = 0;
        video.src = remuxUrl(t);
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

  function setQuality(q: 'orig' | 'auto' | number) {
    panel = '';
    if (q === quality) return;
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

  // ---------- отметки заставки и титров (хост) ----------
  async function mark(which: 'introStart' | 'introEnd' | 'credits' | 'reset') {
    if (!info) return;
    const m = info.markers;
    const body =
      which === 'reset'
        ? { introStart: null, introEnd: null, credits: null }
        : {
            introStart: which === 'introStart' ? time : (m.intro?.[0] ?? null),
            introEnd: which === 'introEnd' ? time : (m.intro?.[1] ?? null),
            credits: which === 'credits' ? time : m.credits,
          };
    try {
      const r = await put<{ markers: PlayInfo['markers'] }>(`/api/files/${fileId}/markers`, body);
      info.markers = r.markers;
      say(which === 'reset' ? 'Отметки сезона сброшены' : 'Сохранено для всего сезона');
    } catch (e) {
      say(e instanceof Error ? e.message : 'Не сохранилось');
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

  // Титры серии — карточка следующей и обратный отсчёт, как у Netflix.
  let ticker: ReturnType<typeof setInterval> | undefined;
  function startCountdown(from: number) {
    clearInterval(ticker);
    countdown = from;
    ticker = setInterval(() => {
      if (countdown === null) return clearInterval(ticker);
      countdown--;
      if (countdown <= 0) {
        clearInterval(ticker);
        goNext();
      }
    }, 1000);
  }

  $effect(() => {
    if (inCredits && !paused) {
      startCountdown(untrack(() => countdown) ?? 10);
      return () => clearInterval(ticker);
    }
    if (!inCredits && !untrack(() => video?.ended)) countdown = null;
  });

  function onEnded() {
    save(true);
    if (info?.next) {
      creditsSkipped = false;
      startCountdown(Math.min(countdown ?? 5, 5));
    }
  }

  function goNext() {
    clearInterval(ticker);
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
    else if (k === 'KeyS' && inIntro && intro) seek(intro[1]);
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
  const thumb = $derived(hover !== null && info?.trickplay ? trickFrame(info.trickplay, hover) : null);
  const thumbLeft = $derived.by(() => {
    if (hover === null || !duration) return 0;
    const w = info?.trickplay?.width ?? 0;
    const x = (hover / duration) * barWidth;
    return Math.max(w / 2, Math.min(barWidth - w / 2, x));
  });
</script>

<svelte:window onkeydown={key} />

<div class="player" class:idle={!shown} bind:this={box} onpointermove={poke} role="application" aria-label="Видеоплеер">
  <!-- svelte-ignore a11y_media_has_caption -->
  <video
    bind:this={video}
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
    onwaiting={onWaiting}
    onplaying={() => (waiting = false)}
    oncanplay={() => (waiting = false)}
    onended={onEnded}
    onerror={onError}
    onclick={toggle}
    ondblclick={fullscreen}
  ></video>

  {#if subtitle}
    <div class="subs" class:up={shown}>{subtitle}</div>
  {/if}

  {#if waiting && !error}
    <div class="spinner" aria-label="Загрузка"></div>
  {/if}

  {#if toast}<div class="toast" role="status">{toast}</div>{/if}

  {#if error}
    <div class="err">
      <Icon name="info" size={40} />
      <p>{error}</p>
      <div class="row">
        <button class="btn" onclick={back}><Icon name="back" size={16} /> Назад</button>
        {#if info?.hls && quality === 'orig'}
          <button class="btn" onclick={() => setQuality('auto')}><Icon name="refresh" size={16} /> Адаптивное качество</button>
        {/if}
        <a class="btn white" href="/api/files/{fileId}/download" download><Icon name="download" size={16} /> Скачать</a>
      </div>
    </div>
  {/if}

  {#if inIntro && intro}
    <button class="skip" class:low={!shown} onclick={() => seek(intro[1])}>Пропустить заставку</button>
  {/if}

  {#if (inCredits || (countdown !== null && !inCredits)) && info?.next}
    <div class="nextcard" class:low={!shown}>
      {#if info.nextInfo?.still}<img src={info.nextInfo.still} alt="" />{/if}
      <div class="nc">
        <span class="muted">Следующая серия</span>
        <b>{info.nextInfo?.label ?? ''}</b>
        <div class="ncb">
          {#if inCredits}<button class="btn sm" onclick={() => ((creditsSkipped = true), clearInterval(ticker), (countdown = null))}>Смотреть титры</button>{/if}
          <button class="btn sm white go" onclick={goNext} style:--p="{countdown === null ? 0 : ((10 - countdown) / 10) * 100}%">
            <Icon name="play" size={13} /> Смотреть{countdown !== null ? ` · ${countdown}` : ''}
          </button>
        </div>
      </div>
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
    <span class="grow"></span>
    {#if session.me?.manage && info}
      <button class="cbtn" class:on={panel === 'marks'} aria-label="Отметить заставку и титры" title="Отметить заставку и титры" onclick={() => (panel = panel === 'marks' ? '' : 'marks')}>
        <Icon name="flag" size={20} />
      </button>
    {/if}
  </div>

  {#if paused && !waiting && !error && info}
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
      bind:clientWidth={barWidth}
      onpointerdown={barDown}
      onpointermove={barMove}
      onpointerup={barUp}
      onpointerleave={() => !dragging && (hover = null)}
    >
      <div class="rail">
        <i class="buf" style:width="{bufferedPct}%"></i>
        {#if intro && duration}<i class="introzone" style:left="{(intro[0] / duration) * 100}%" style:width="{((intro[1] - intro[0]) / duration) * 100}%"></i>{/if}
        <i class="played" style:width="{dragging && hover !== null ? (hover / duration) * 100 : playedPct}%"></i>
      </div>
      {#if hover !== null && duration}
        {#if thumb && info?.trickplay}
          <span
            class="thumb"
            style:left="{thumbLeft}px"
            style:width="{info.trickplay.width}px"
            style:height="{info.trickplay.height}px"
            style:background-image="url(/api/files/{fileId}/trickplay/{thumb.sheet}.jpg)"
            style:background-position="-{thumb.x}px -{thumb.y}px"
          ><span>{clock(hover)}</span></span>
        {:else}
          <span class="tip" style:left="{(hover / duration) * 100}%">{clock(hover)}</span>
        {/if}
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
      {#if info?.hls}
        <button class="cbtn q" class:on={panel === 'quality'} aria-label="Качество" title="Качество" onclick={() => (panel = panel === 'quality' ? '' : 'quality')}>
          {qualityLabel}
        </button>
      {/if}
      {#if info?.next}
        <button class="cbtn" aria-label="Следующая серия" title="Следующая серия" onclick={goNext}><Icon name="next" size={22} /></button>
      {/if}
      <button class="cbtn" aria-label="Во весь экран" onclick={fullscreen}><Icon name={full ? 'exitfs' : 'fullscreen'} size={22} /></button>
    </div>
  </div>

  {#if panel && info}
    <div class="panel" class:top={panel === 'marks'} role="menu">
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
      {:else if panel === 'quality'}
        <h4>Качество</h4>
        {#if origOk}
          <button role="menuitemradio" aria-checked={quality === 'orig'} class:on={quality === 'orig'} onclick={() => setQuality('orig')}>
            <Icon name="check" size={16} /> Оригинал <small>как в файле</small>
          </button>
        {/if}
        <button role="menuitemradio" aria-checked={quality === 'auto'} class:on={quality === 'auto'} onclick={() => setQuality('auto')}>
          <Icon name="check" size={16} /> Авто <small>под скорость интернета</small>
        </button>
        {#each ladder as l (l)}
          <button role="menuitemradio" aria-checked={quality === l} class:on={quality === l} onclick={() => setQuality(l)}>
            <Icon name="check" size={16} /> {l}p
          </button>
        {/each}
      {:else}
        <h4>Отметки для всего {info.season ? `${info.season} сезона` : 'фильма'}</h4>
        <p class="hint">Перемотайте к нужному месту и нажмите. Зрители увидят «Пропустить заставку» и следующую серию на титрах.</p>
        <button onclick={() => mark('introStart')}><Icon name="flag" size={16} /> Заставка началась · {clock(time)}{#if intro}<small>{clock(intro[0])}</small>{/if}</button>
        <button onclick={() => mark('introEnd')}><Icon name="flag" size={16} /> Заставка закончилась · {clock(time)}{#if intro}<small>{clock(intro[1])}</small>{/if}</button>
        <button onclick={() => mark('credits')}><Icon name="flag" size={16} /> Начались титры · {clock(time)}{#if credits !== null}<small>{clock(credits)}</small>{/if}</button>
        {#if info.markers.manual}<button onclick={() => mark('reset')}><Icon name="trash" size={16} /> Сбросить отметки</button>{/if}
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
  .grow {
    flex: 1;
  }
  .top .cbtn {
    flex: none;
  }
  .introzone {
    background: rgba(255, 255, 255, 0.25);
  }
  .thumb {
    position: absolute;
    bottom: 26px;
    transform: translateX(-50%);
    border-radius: 8px;
    border: 2px solid #fff;
    background-repeat: no-repeat;
    box-shadow: 0 12px 30px -6px rgba(0, 0, 0, 0.8);
    pointer-events: none;
  }
  .thumb span {
    position: absolute;
    left: 50%;
    bottom: -26px;
    transform: translateX(-50%);
    padding: 2px 8px;
    border-radius: 6px;
    background: rgba(20, 17, 43, 0.9);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .skip,
  .nextcard {
    position: absolute;
    right: clamp(14px, 3vw, 36px);
    bottom: calc(110px + env(safe-area-inset-bottom));
    transition: bottom 0.3s;
  }
  .skip.low,
  .nextcard.low {
    bottom: calc(40px + env(safe-area-inset-bottom));
  }
  .skip {
    padding: 12px 22px;
    border-radius: 8px;
    border: 1.5px solid rgba(255, 255, 255, 0.8);
    background: rgba(20, 17, 43, 0.75);
    backdrop-filter: blur(8px);
    color: #fff;
    font-weight: 700;
    font-size: 15px;
    cursor: pointer;
    animation: slidein 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .skip:hover {
    background: #fff;
    color: #0a0a0a;
  }
  @keyframes slidein {
    from {
      opacity: 0;
      transform: translateX(20px);
    }
  }
  .nextcard {
    display: flex;
    gap: 14px;
    width: min(440px, calc(100vw - 28px));
    padding: 12px;
    border-radius: 14px;
    background: rgba(20, 17, 43, 0.94);
    border: 1px solid var(--line);
    box-shadow: 0 20px 50px -10px #000;
    animation: slidein 0.4s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .nextcard img {
    width: 150px;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    border-radius: 8px;
    flex: none;
  }
  .nc {
    display: grid;
    align-content: center;
    gap: 4px;
    min-width: 0;
    font-size: 13px;
  }
  .nc b {
    font-size: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ncb {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }
  .go {
    background: linear-gradient(90deg, #d8d4f0 var(--p), #fff var(--p));
  }
  .toast {
    position: absolute;
    top: calc(80px + env(safe-area-inset-top));
    left: 50%;
    transform: translateX(-50%);
    padding: 10px 18px;
    border-radius: 999px;
    background: rgba(20, 17, 43, 0.94);
    border: 1px solid var(--line);
    font-size: 14px;
    animation: slidein 0.3s ease;
  }
  .panel.top {
    top: calc(76px + env(safe-area-inset-top));
    bottom: auto;
    max-width: 340px;
  }
  .panel small {
    margin-left: auto;
    color: var(--muted);
    font-size: 12px;
  }
  .panel .hint {
    margin: 0 10px 8px;
    font-size: 12px;
    color: var(--muted);
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
