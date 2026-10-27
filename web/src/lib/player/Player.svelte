<script lang="ts" module>
  import type { Snippet } from 'svelte';

  /** Совместный просмотр: свои нажатия - в комнату, команды комнаты - сюда. */
  export interface PlayerSync {
    /** Зритель сам: пауза/смотреть, перемотка, следующая серия (`fileId`). */
    local(s: { playing: boolean; position: number; fileId?: number }): void;
    /** Команды комнаты; вернуть - отписка. */
    listen(apply: (s: { playing: boolean; position: number }) => void): () => void;
    /** «Назад» из комнаты. */
    back?: () => void;
  }
</script>

<script lang="ts">
  import { artW } from '$lib/ui/art';
  import { goto } from '$app/navigation';
  import { untrack } from 'svelte';
  import { api } from '$lib/api';
  import { clock } from '$lib/fmt';
  import { localized } from '$lib/localized.svelte';
  import type { PlayInfo, FileItem, TitleDetail } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import type HlsType from 'hls.js';
  import { levels, trickFrame } from './levels';
  import { nativeLevels } from './native-master';
  import { qualityTier } from './quality-label';
  import { abrEstimate, forgetSpeed, measureSpeed, startQuality } from './speed';
  import { cueAt, parseVtt, type Cue } from './vtt';
  import { forgetCatalog } from '$lib/catalog.svelte';
  import { extChanged } from '$lib/discover/sheet.svelte';
  import { noteOwnSave, progressSync } from '$lib/progressSync.svelte';


  // `src` - видео не с компьютера хоста (Rutube, VK, свой сайт, прямая ссылка - поток через сервер):
  // тот же плеер, без прогресса, субтитров и озвучек.
  let {
    fileId = 0,
    src = null,
    onfail,
    sync = null,
    alts = [],
    onalt,
    onwrong,
    expect = null,
    nextSource = null, onnext, onfinish, onNearEnd,
    children,
  }: {
    fileId?: number;
    /** `linkId` - ссылка, у которой сохраняем место; `start` - откуда начать (где остановился). */
    src?: { url: string; hls: boolean; title: string; linkId?: number; start?: number } | null;
    onfail?: () => void;
    sync?: PlayerSync | null;
    /** Видео по ссылке: другие видео этого фильма (другая озвучка) - меню «Озвучка». */
    alts?: { id: number; label: string }[];
    onalt?: (id: number) => void;
    /** Видео по ссылке оказалось не тем фильмом - убрать источник и взять следующий. */
    onwrong?: () => void;
    /** Длительность фильма (с): видео заметно короче или длиннее - похоже, это не он. */
    expect?: number | null;
    nextSource?: { id: number; title: string } | null;
    onnext?: () => void;
    onfinish?: () => void;
    onNearEnd?: () => void;
    children?: Snippet;
  } = $props();

  let info = $state<PlayInfo | null>(null);
  let video: HTMLVideoElement | undefined = $state();
  let box: HTMLDivElement | undefined = $state();
  let error = $state('');
  let toast = $state('');

  // Источник: файл как есть, перепаковка с места `offset` (время потока ffmpeg с нуля) или
  // адаптивный HLS (как у Netflix: качество под скорость интернета).
  let mode = $state<'raw' | 'remux' | 'copy' | 'hls'>('raw');
  let quality = $state<'orig' | 'auto' | number>('orig');
  let hlsHeight = $state<number | null>(null);
  let offset = $state(0);
  let audio = $state(0);
  let hls: HlsType | null = null;
  // Скорость до хоста у этого зрителя (Мбит/с) и сколько раз видео останавливалось - для выбора
  // качества и для «Управления» у хоста. Замер - сразу, параллельно со сведениями о файле.
  let mbit = $state<number | null>(null);
  let stallCount = 0;
  // Уже подвисало у этого зрителя - следующие серии сразу в автокачестве.
  let slow = false;
  const speed = typeof window === 'undefined' ? Promise.resolve(null) : measureSpeed();
  let pendingSeek = 0;

  let paused = $state(true);
  let current = $state(0);
  let buffered = $state(0);
  let waiting = $state(true);
  let volume = $state(1);
  let lastVolume = 1;
  let muted = $state(false);
  let full = $state(false);
  let shown = $state(true);
  let panel = $state<'' | 'audio' | 'subs' | 'quality' | 'speed' | 'keys' | 'settings' | 'episodes' | 'sources'>('');
  let episodeFiles = $state<FileItem[]>([]);
  let episodeSeason = $state(1);
  let episodesBusy = $state(false);
  async function openEpisodes() {
    if (panel === 'episodes') { panel = ''; return; }
    panel = 'episodes'; episodeSeason = info?.season ?? 1;
    if (!info?.titleId || episodeFiles.length) return;
    episodesBusy = true;
    try { const detail = await api<TitleDetail>(`/api/titles/${info.titleId}`); episodeFiles = detail.files; } catch { say('Не удалось загрузить серии'); }
    finally { episodesBusy = false; }
  }
  function pickEpisode(id: number) { save(true); panel = ''; if (id === fileId) return; if (sync) sync.local({ playing: true, position: 0, fileId: id }); else goto(`/watch/${id}`, { replace: true }); }
  let rate = $state(1);
  let jump = $state<{ side: 'left' | 'right'; n: number; key: number } | null>(null);
  const pipOk = typeof document !== 'undefined' && !!document.pictureInPictureEnabled;
  const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];
  let hover = $state<number | null>(null);
  let barWidth = $state(0);
  let countdown = $state<number | null>(null);
  let ended = $state(false);
  let nextPrepared = false;
  const canNext = $derived(!!info?.next || !!nextSource);
  let creditsSkipped = $state(false);

  let subId = $state('');

  // Вид субтитров - у каждого зрителя свой, помнится на устройстве.
  type SubLook = { size: 's' | 'm' | 'l'; look: 'box' | 'shadow' | 'yellow' | 'cinema'; high: boolean };
  const SUB_SIZES = [
    { id: 's', label: 'Мелкие' },
    { id: 'm', label: 'Средние' },
    { id: 'l', label: 'Крупные' },
  ] as const;
  const SUB_LOOKS = [
    { id: 'box', label: 'На плашке' },
    { id: 'shadow', label: 'С тенью' },
    { id: 'cinema', label: 'Обводка' },
    { id: 'yellow', label: 'Жёлтые' },
  ] as const;
  function savedLook(): SubLook {
    const base: SubLook = { size: 'm', look: 'box', high: false };
    try {
      const saved = JSON.parse(localStorage.getItem('fsfy-subs-look') ?? 'null');
      return saved && typeof saved === 'object' ? { ...base, ...saved } : base;
    } catch {
      return base;
    }
  }
  let subLook = $state<SubLook>(savedLook());
  function setLook(p: Partial<SubLook>) {
    subLook = { ...subLook, ...p };
    try {
      localStorage.setItem('fsfy-subs-look', JSON.stringify(subLook));
    } catch {
      /* приватный режим */
    }
  }
  let cues = $state<Cue[]>([]);

  const hevcOk =
    typeof document !== 'undefined' && document.createElement('video').canPlayType('video/mp4; codecs="hvc1"') !== '';
  const nativeHls =
    typeof document !== 'undefined' &&
    document.createElement('video').canPlayType('application/vnd.apple.mpegurl') !== '' &&
    !('MediaSource' in globalThis && /Chrome|Android/.test(navigator.userAgent));

  // Длительность из самого видео - состоянием (свойство <video> интерфейс не отслеживает: у видео по
  // ссылке без неё таймлайн стоял и перемотка не работала).
  let mediaDuration = $state(0);
  const duration = $derived(info?.play.duration || (mediaDuration ? offset + mediaDuration : 0));
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
    return (p.direct && audio === 0 && (!p.hevc || hevcOk)) || (p.remux && !p.transcode && (!p.hevc || hevcOk)) || (!p.remux && !info?.hls);
  });
  const ladder = $derived(levels(info?.play.height ?? null));
  // Видео по ссылке (Rutube, VK, свой сайт): качества - из самого потока (hls.js), «Авто» - -1.
  let srcLevels = $state<{ i: number; height: number; label?: number }[]>([]);
  let srcLevel = $state(-1);
  let nativePaused: boolean | null = null;
  let desiredPlaying = true;
  let switching = false;
  let roomBusy = $state(false);
  async function watchTogether() {
    if(roomBusy || sync || (!fileId && !src?.linkId))return;
    roomBusy=true;
    try { const room = await api<{id:string}>('/api/rooms',{method:'POST',json:{...(src?.linkId ? {linkId:src.linkId}:{fileId}),position:time}});
      video?.pause(); if(document.fullscreenElement)await document.exitFullscreen().catch(()=>{});
      await goto(`/room/${room.id}`);
    } catch {say('Не удалось начать совместный просмотр');} finally {roomBusy=false;}
  }
  function pickLevel(i: number) {
    panel = '';
    srcLevel = i;
    if (hls) { if(video?.paused) hls.loadLevel = i; else hls.currentLevel = i; }
    else if (nativeHls && src?.hls && video) {
      const url = new URL(i === -1 ? src.url : `/api/links/${src.linkId}/stream/index.m3u8`, location.href);
      const height = srcLevels.find(l => l.i === i)?.height;
      if (height) url.searchParams.set('height', String(height));
      pendingSeek = video.currentTime;
      nativePaused = video.paused;
      switching = true;
      video.src = url.href;
    }
    say(i === -1 ? 'Качество - автоматически' : `Качество ${srcLevels.find((l) => l.i === i)?.label ?? qualityTier(srcLevels.find((l) => l.i === i)?.height ?? 720)}p`);
  }
  const qualityLabel = $derived(
    src
      ? srcLevel === -1
        ? `Авто${hlsHeight ? ` · ${qualityTier(hlsHeight)}p` : ''}`
        : `${srcLevels.find((l) => l.i === srcLevel)?.label ?? qualityTier(srcLevels.find((l) => l.i === srcLevel)?.height ?? 720)}p`
      : mode === 'hls'
        ? quality === 'auto'
          ? `Авто${hlsHeight ? ` · ${qualityTier(hlsHeight)}p` : ''}`
          : `${quality}p`
        : 'Оригинал',
  );

  function remuxUrl(t: number) {
    const q = new URLSearchParams({ t: t.toFixed(2), audio: String(audio) });
    if (hevcOk) q.set('hevc', '1');
    return `/api/files/${fileId}/play?${q}`;
  }

  let manifestRequest: AbortController | null = null;
  let loadGeneration = 0;
  function destroyHls() {
    loadGeneration++;
    manifestRequest?.abort();
    manifestRequest = null;
    hls?.destroy();
    hls = null;
    hlsHeight = null;
  }

  async function load(at: number) {
    if (!info || !video) return;
    destroyHls();
    const generation=loadGeneration;
    switching = true;
    error = '';
    const p = info.play;
    if (quality === 'orig' && !origOk && info.hls) quality = 'auto';
    if (quality !== 'orig' && !info.hls) quality = 'orig';
    waiting = true;
    let url = src?.url ?? '';
    if (src) {
      offset = 0;
      mode = src.hls ? 'hls' : 'raw';
      if (!src.hls) {
        pendingSeek = at;
        video.src = src.url;
        return;
      }
    } else if (quality === 'orig') {
      const direct = (p.direct && audio === 0 && (!p.hevc || hevcOk)) || !p.remux;
      if (direct) {
        mode = 'raw';
        offset = 0;
        pendingSeek = at;
        video.src = `/api/files/${fileId}/raw`;
        return;
      }
      if (!nativeHls) {
        // Chrome, Edge, окно на Windows: поток перепаковки - без ожидания, перемотка - новым потоком.
        mode = 'remux';
        offset = at;
        pendingSeek = 0;
        video.src = remuxUrl(at);
        return;
      }
      // Safari и окно на Mac ходят по Range - им готовая копия у хоста кусочками HLS.
      mode = 'copy';
    } else {
      mode = 'hls';
    }
    if (!src) offset = 0;
    if (!src)
      url =
      mode === 'copy'
        ? `/api/files/${fileId}/copy/${audio}/index.m3u8?from=${Math.floor(at)}`
        : quality === 'auto'
          ? `/api/files/${fileId}/hls/master.m3u8?audio=${audio}`
          : `/api/files/${fileId}/hls/${quality}/index.m3u8?audio=${audio}`;
    if (nativeHls) {
      srcLevels = []; srcLevel = -1;
      if (src?.hls && src.linkId) {
        const request = new AbortController(); manifestRequest = request;
        fetch(`/api/links/${src.linkId}/stream/index.m3u8`, {signal: request.signal}).then(async response => {
          if (!response.ok) return;
          const qualities = nativeLevels(await response.text());
          if (!request.signal.aborted) srcLevels = qualities;
        }).catch(() => {});
      }
      pendingSeek = at;
      video.src = url;
      return;
    }
    const { default: Hls } = await import('hls.js/light');
    if(generation!==loadGeneration)return;
    if (!Hls.isSupported()) {
      error = 'Этот браузер не умеет адаптивное видео - скачайте файл или откройте в другом браузере';
      return;
    }
    let retries = 0;
    const h = new Hls({
      startPosition: at,
      // Запас вперёд - до минуты: короткие провалы скорости не останавливают видео.
      maxBufferLength: src ? 20 : 60,
      maxMaxBufferLength: 120,
      startLevel: srcLevel,
      // Автокачество начинает с уровня под замеренную скорость, а не наугад.
      ...(src || abrEstimate(mbit) === undefined ? {} : { abrEwmaDefaultEstimate: abrEstimate(mbit) }),
      capLevelToPlayerSize: true,
      backBufferLength: 60,
      // Плейлист у хоста может готовиться несколько секунд (копия, перекодирование).
      manifestLoadPolicy: {
        default: {
          maxTimeToFirstByteMs: 60_000,
          maxLoadTimeMs: 90_000,
          timeoutRetry: { maxNumRetry: 2, retryDelayMs: 1000, maxRetryDelayMs: 4000 },
          errorRetry: { maxNumRetry: 3, retryDelayMs: 1000, maxRetryDelayMs: 8000 },
        },
      },
      playlistLoadPolicy: {
        default: {
          maxTimeToFirstByteMs: 60_000,
          maxLoadTimeMs: 90_000,
          timeoutRetry: { maxNumRetry: 2, retryDelayMs: 1000, maxRetryDelayMs: 4000 },
          errorRetry: { maxNumRetry: 3, retryDelayMs: 1000, maxRetryDelayMs: 8000 },
        },
      },
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
    h.on(Hls.Events.MANIFEST_PARSED, () => {
      // Разные качества (без повторов высоты) - от лучшего.
      const seen = new Set<number>();
      srcLevels = h.levels
        .map((l, i) => ({ i, height: l.height, label: qualityTier(l.height,l.width) }))
        .filter((l) => l.height && !seen.has(l.label) && seen.add(l.label))
        .sort((a, b) => b.height - a.height);
      switching = false;
      if (srcLevel >= 0) h.currentLevel = srcLevel;
      if(desiredPlaying) video?.play().catch(() => (paused = true));

    });
    h.on(Hls.Events.ERROR, (_e, d) => {
      if (!d.fatal || !desiredPlaying) return;
      if (retries++ < 3) {
        if (d.type === Hls.ErrorTypes.MEDIA_ERROR) h.recoverMediaError();
        else h.startLoad();
        return;
      }
      error = src
        ? 'Видео с сайта не загружается - откройте его плеером сайта'
        : d.response?.code === 503
          ? 'Сервер Nova сейчас занят другими просмотрами - попробуйте через минуту'
          : 'Видео не загружается. Проверьте интернет или скачайте файл';
    });
    h.loadSource(url);
    h.attachMedia(video);
  }

  let resourceGeneration = 0;
  const sourceIdentity = $derived(src ? `link:${src.linkId ?? ''}:${src.url}` : `file:${fileId}`);
  $effect(() => {
    void sourceIdentity;
    return untrack(() => {
    const resource = ++resourceGeneration;
    const id = fileId;
    const s = src;
    untrack(() => {srcLevel = -1; srcLevels = []; desiredPlaying = true; nativePaused = null;});
    ready = false;
    current = 0; offset = 0; mediaDuration = 0; buffered = 0;
    waiting = true; paused = true; lastSaved = 0;
    info = null;
    error = '';
    recovered = 0;
    countdown = null;
    ended = false; nextPrepared = false;
    creditsSkipped = false;
    subId = '';
    cues = [];
    if (s) {
      // Внешний источник: без запроса к серверу - и без отслеживания (load читает и меняет состояние).
      untrack(() => {
        info = {
          play: {
            direct: !s.hls,
            remux: false,
            transcode: false,
            hevc: false,
            duration: null,
            width: null,
            height: null,
            audio: [],
            subs: [],
          },
          titleId: 0, kind: 'movie', title: s.title, episode: '', prev: null, next: null, nextInfo: null, season: null, position: 0,
          markers: { intro: null, credits: null, manual: false }, trickplay: null, hls: false,
        };
        load(s.start && s.start > 5 ? s.start : 0);
      });
      return destroyHls;
    }
    api<PlayInfo>(`/api/files/${id}/info`).then(
      async (i) => {
        if(resource!==resourceGeneration)return;
        // Замер сам укладывается в 2,5 с (медленно - считает по пришедшему); 3 с - на всякий случай.
        mbit = await Promise.race([speed, new Promise<null>((r) => setTimeout(() => r(null), 3000))]);
        if(resource!==resourceGeneration)return;
        info = i;
        audio = Math.max(0, i.play.audio.findIndex((a) => a.default));
        quality = slow ? 'auto' : startQuality(mbit, i.bitrate ?? null);
        if (quality === 'auto' && i.hls) say(`Интернет ${mbit} Мбит/с - включили автокачество`);
        const forced = i.play.subs.find((s) => s.forced && s.lang?.startsWith('ru'));
        if (forced) pickSubs(forced.id);
        const start = sync && remote ? remote.position + (remote.playing ? (performance.now()-remoteAt)/1000 : 0) : i.position > 5 && (!i.play.duration || i.position < i.play.duration - 20) ? i.position : 0;
        load(start);
      },
      () => {if(resource===resourceGeneration)error = 'Не удалось открыть файл';},
    );
    return destroyHls;
    });
  });

  function onLoaded() {
    if (!video) return;
    try {
      video.volume = volume;
    } catch {
      /* iOS не даёт менять громкость */
    }
    // Новый источник (перемотка перепаковки, другое качество) - скорость та же.
    video.playbackRate = rate;
    if (pendingSeek > 0) video.currentTime = pendingSeek;
    pendingSeek = 0;
    switching = false;
    // В комнате играет и стоит то, что у всех: сначала - состояние комнаты.
    if (sync) {
      ready = true;
      // Пока грузилось, у остальных видео шло дальше.
      if (remote) applyRemote({ ...remote, position: remote.playing ? remote.position + (performance.now() - remoteAt) / 1000 : remote.position });
      return;
    }
    if (!hls) {
      if (nativePaused === true || !desiredPlaying) video.pause();
      else video.play().catch(() => (paused = true));
      nativePaused = null;
    }
  }

  // ---------- совместный просмотр ----------
  let ready = false;
  let remote: { playing: boolean; position: number } | null = null;
  let remoteAt = 0;
  // Команды комнаты в комнату не отправляем (seek(…, false)); свои нажатия уходят всегда - раньше
  // сверка раз в 5 с на 0,6 с глушила их, и пауза в это окно не доходила до остальных.
  function applyRemote(s: { playing: boolean; position: number }) {
    remote = s;
    remoteAt = performance.now();
    desiredPlaying = s.playing;
    if (!video || !ready || !duration) return;
    // Большой разрыв - перемотка (у перепаковки это новый поток - не дёргаем по мелочи).
    const difference = Math.abs(time - s.position);
    if (difference > (mode === 'remux' ? 6 : 2.5) && !video.seeking) seek(s.position, false);
    if (s.playing) {hls?.resumeBuffering();if(video.paused)playRemote();}
    if (!s.playing && !video.paused) video.pause();
  }

  // Браузер не даёт запустить видео со звуком без нажатия (зашёл по ссылке) - без звука, и
  // кнопка «Включить звук»: иначе у этого зрителя стоит, у остальных идёт.
  let mutedByBrowser = $state(false);
  function playRemote() {
    video?.play().catch((e: DOMException) => {
      if (!video || e?.name !== 'NotAllowedError') return void (paused = true);
      video.muted = true;
      mutedByBrowser = true;
      video.play().catch(() => (paused = true));
    });
  }
  function unmute() {
    if (!video) return;
    video.muted = false;
    mutedByBrowser = false;
  }

  $effect(() => {
    if (!sync) return;
    return untrack(() => sync.listen((s) => applyRemote(s)));
  });

  $effect(() => {
    const title = src?.title || info?.title;
    if (!title || !('mediaSession' in navigator)) return;
    navigator.mediaSession.metadata = new MediaMetadata({ title, artist: 'Nova', artwork: [{src: new URL('/media-artwork.png', location.href).href, sizes: '512x512', type: 'image/png'}] });
    return () => { navigator.mediaSession.metadata = null; };
  });

  let recovered = 0;
  function onError() {
    if (!info || !video?.error || hls) return;
    // Сбой связи или туннеля - тихо пробуем ещё раз с того же места, без окна ошибки.
    if (desiredPlaying && mode !== 'raw' && recovered++ < 2) {
      load(time);
      return;
    }
    // Браузер не смог сам - адаптивный поток или перепаковка через ffmpeg у хоста.
    // Без перекодирования, если можно: перекодирование нагружает компьютер хоста.
    if (desiredPlaying && mode === 'raw' && (info.hls || info.play.remux)) {
      info.play.direct = false;
      load(time);
      return;
    }
    if (src) {
      error = 'Видео с сайта не воспроизводится - откройте его плеером сайта';
      return;
    }
    error = info.play.remux
      ? 'Видео не воспроизводится. Попробуйте ещё раз или скачайте файл'
      : 'Этот формат браузер не играет. Скачайте файл или напишите администрации Nova';
  }

  // Медленный интернет: остановка дольше 3 секунд или две за 30 секунд - включаем автокачество
  // с того же места (и в Safari тоже). В автокачестве уровень снижает сам hls.js.
  let stalls: number[] = [];
  let stallTimer: ReturnType<typeof setTimeout> | undefined;
  const adaptive = () => mode === 'hls' && quality === 'auto';
  function slowDown() {
    clearTimeout(stallTimer);
    stalls = [];
    slow = true;
    forgetSpeed();
    quality = 'auto';
    say('Медленный интернет - включили автокачество');
    load(time);
  }
  function onWaiting() {
    if (!desiredPlaying || video?.paused) {waiting=false;return;}
    waiting = true;
    if (!video || video.seeking || current < 2) return;
    stallCount++;
    if (!info?.hls || src || adaptive()) return;
    const now = Date.now();
    stalls = [...stalls.filter((t) => now - t < 30_000), now];
    if (stalls.length >= 2) return slowDown();
    clearTimeout(stallTimer);
    stallTimer = setTimeout(() => {
      if (waiting && desiredPlaying && !video?.seeking && !adaptive()) slowDown();
    }, 3_000);
  }

  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function say(text: string) {
    clearTimeout(toastTimer);
    toast = text;
    toastTimer = setTimeout(() => (toast = ''), 2200);
  }

  function seek(t: number, report = true) {
    if (!video || !duration) return;
    t = Math.max(0, Math.min(duration - 1, t));
    if (mode !== 'remux') {
      video.currentTime = t;
    } else {
      const local = t - offset;
      // Внутри уже полученного куска - обычная перемотка, иначе новый поток с нужного места.
      if (local >= 0 && local <= buffered) video.currentTime = local;
      else {
        offset = t;
        current = 0;
        video.src = remuxUrl(t);
        waiting = true;
      }
    }
    if (report && sync) {
      const playing = remote ? remote.playing : !paused;
      remote = { playing, position: t };
      remoteAt = performance.now();
      sync.local({ playing, position: t });
    }
    poke();
  }

  function toggle() {
    if (!video) return;
    // В комнате «пауза или пуск» решает состояние комнаты, а не своё видео: у зрителя оно могло
    // стоять (браузер не дал запустить, ещё грузится) - и «пауза» запускала видео у всех.
    const play = sync && remote ? !remote.playing : video.paused;
    desiredPlaying = play;
    if(play)hls?.resumeBuffering();
    if (play) (sync ? playRemote() : video.play().catch(() => {}));
    else video.pause();
    if (sync) {
      remote = { playing: play, position: time };
      remoteAt = performance.now();
      sync.local({ playing: play, position: time });
    }
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

  // ---------- прогресс ----------
  let lastSaved = 0;
  let saving: Promise<unknown> = Promise.resolve();
  function save(force = false) {
    if (!duration || time < 1) return;
    // Видео по ссылке - место у ссылки ("Продолжить просмотр" у фильма из Обзора).
    const url = src ? (src.linkId ? `/api/links/${src.linkId}/progress` : null) : `/api/progress/${fileId}`;
    if (!url) return;
    const now = Date.now();
    if (!force && now - lastSaved < 5_000) return;
    lastSaved = now;
    noteOwnSave();
    // Как идёт просмотр - хосту в «Управление»: скорость, качество, остановки.
    const shown = quality === 'orig' ? 'orig' : hlsHeight ? `${hlsHeight}p` : quality === 'auto' ? 'auto' : `${quality}p`;
    const net = src ? undefined : { mbit, stalls: stallCount, quality: shown };
    const body = JSON.stringify({ position: time, duration, net });
    saving = saving.catch(() => {}).then(() => fetch(url, {
      method: 'PUT',
      keepalive: true,
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-fsfy': '1' },
      body,
    })).catch(() => {});
    return saving;
  }

  $effect(() => {
    const onHide = () => save(true);
    window.addEventListener('pagehide', onHide);
    document.addEventListener('visibilitychange', onHide);
    return () => {
      save(true);
      forgetCatalog();
      extChanged();
      window.removeEventListener('pagehide', onHide);
      document.removeEventListener('visibilitychange', onHide);
    };
  });

  // Смотрели на другом устройстве (ноутбук закрыли - досматриваем на ПК): предложить перейти туда,
  // где остановились. В комнате место задаёт комната.
  let elsewhere = $state<number | null>(null);
  // Видео по ссылке сильно отличается по длине от фильма (больше 12%) - предупредить.
  let mismatchOff = $state(false);
  const mismatch = $derived(!!src && !mismatchOff && !!expect && duration > 60 && Math.abs(duration - expect) / expect > 0.12);
  // Высота верхней и нижней панелей - для того, что лежит поверх видео (комната, реакции): не наезжать.
  let topH = $state(80);
  let bottomH = $state(120);
  $effect(() => {
    if (!progressSync.stamp || sync) return;
    untrack(() => void checkElsewhere());
  });
  async function checkElsewhere() {
    const url = src ? (src.linkId ? `/api/links/${src.linkId}/resume` : null) : `/api/progress/${fileId}`;
    if (!url || !duration) return;
    try {
      const r = await api<{ position: number | null }>(url);
      if (r.position != null && Math.abs(r.position - time) > 20 && r.position < duration - 30) elsewhere = r.position;
    } catch {
      /* не вышло - останемся на своём месте */
    }
  }
  function goElsewhere() {
    if (elsewhere === null) return;
    seek(elsewhere);
    elsewhere = null;
  }

  function onTime() {
    if (src && duration > 0 && time >= duration - 90 && !nextPrepared) { nextPrepared = true; onNearEnd?.(); }
    if (!video) return;
    current = video.currentTime;
    if (video.buffered.length) buffered = video.buffered.end(video.buffered.length - 1);
    save();
  }

  // Титры серии - карточка следующей и обратный отсчёт, как у Netflix.
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

  $effect(() => { if (ended && nextSource && !creditsSkipped) startCountdown(5); });

  function onEnded() {
    ended = true; onfinish?.();
    save(true);
    if (canNext) {
      creditsSkipped = false;
      startCountdown(Math.min(countdown ?? 5, 5));
    }
  }

  function goNext() {
    clearInterval(ticker);
    countdown = null;
    if (nextSource) { onnext?.(); return; }
    if (!info?.next) return;
    // В комнате следующая серия - у всех сразу: комната сменит файл.
    if (sync) sync.local({ fileId: info.next, playing: true, position: 0 });
    else goto(`/watch/${info.next}`, { replace: true });
  }

  async function back() {
    await Promise.race([save(true), new Promise((resolve) => setTimeout(resolve, 300))]);
    if (sync?.back) return sync.back();
    if (history.length > 1) history.back();
    else goto(info?.titleId ? `/title/${info.titleId}` : '/discover');
  }

  // ---------- интерфейс ----------
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let controlsHover = false;
  function poke(event?: Event) {
    if (event instanceof PointerEvent && event.type === 'pointermove') controlsHover = !!(event.target instanceof Element && event.target.closest('.bottom'));
    shown = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (!paused && !panel && !controlsHover && !dragging && !box?.querySelector('.bottom :focus-visible')) shown = false;
    }, 2800);
  }

  function fullscreen() {
    if (document.fullscreenElement) document.exitFullscreen();
    else if (box?.requestFullscreen) box.requestFullscreen().catch(() => {});
    else (video as HTMLVideoElement & { webkitEnterFullscreen?: () => void })?.webkitEnterFullscreen?.();
  }

  function setRate(r: number) {
    rate = Math.max(0.25, Math.min(3, Math.round(r * 100) / 100));
    if (video) video.playbackRate = rate;
    say(rate === 1 ? 'Обычная скорость' : `Скорость ×${String(rate).replace('.', ',')}`);
  }

  async function pip() {
    try {
      if (document.pictureInPictureElement) await document.exitPictureInPicture();
      else await video?.requestPictureInPicture();
    } catch {
      say('Картинка в картинке недоступна');
    }
  }

  // Касания: двойное - ±10 с у левого/правого края (как в приложениях), одно - показать или
  // спрятать управление. Мышь: щелчок - пауза, двойной - во весь экран.
  let lastTap = { at: 0, x: 0 };
  let tapTimer: ReturnType<typeof setTimeout> | undefined;
  function tap(e: PointerEvent) {
    if (e.pointerType === 'mouse') return toggle();
    const w = (e.currentTarget as HTMLElement).clientWidth;
    const side = e.clientX < w / 3 ? 'left' : e.clientX > (w * 2) / 3 ? 'right' : null;
    const now = performance.now();
    if (side && now - lastTap.at < 300 && Math.abs(e.clientX - lastTap.x) < 80) {
      clearTimeout(tapTimer);
      seek(time + (side === 'right' ? 10 : -10));
      jump = { side, n: jump?.side === side ? jump.n + 10 : 10, key: now };
      setTimeout(() => jump?.key === now && (jump = null), 650);
      lastTap = { at: now, x: e.clientX };
      return;
    }
    lastTap = { at: now, x: e.clientX };
    clearTimeout(tapTimer);
    tapTimer = setTimeout(() => {
      if (shown && !paused) {
        clearTimeout(hideTimer);
        shown = false;
      } else poke();
    }, side ? 260 : 0);
  }

  function setVolume(v: number) {
    volume = Math.max(0, Math.min(1, v));
    if (volume > 0) lastVolume = volume;
    muted = volume === 0;
    say(muted ? 'Звук выключен' : `Громкость ${Math.round(volume * 100)}%`);
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
      const saved = localStorage.getItem('fsfy-volume');
      const v = Number(saved);
      if (saved !== null && v >= 0 && v <= 1) { volume = v; muted = v === 0; if (v > 0) lastVolume = v; }
    } catch {
      /* нет хранилища */
    }
    const native = (window as Window & {__NOVA_DESKTOP__?:boolean}).__NOVA_DESKTOP__;
    const nativeFullscreen = (enabled:boolean) => { if(native)void api('/api/host/fullscreen',{method:'POST',json:{enabled}}).catch(()=>{}); };
    const fs = () => {
      full = !!document.fullscreenElement;
      if(full){controlsHover=false;(box?.querySelector(':focus') as HTMLElement|null)?.blur();poke();}
      nativeFullscreen(full);
    };
    document.addEventListener('fullscreenchange', fs);
    return () => {document.removeEventListener('fullscreenchange', fs);if(full)nativeFullscreen(false);};
  });

  function key(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.target instanceof HTMLElement && e.target.closest('input, textarea, [contenteditable="true"]')) {
      // Фокус остался в поле (окно ссылки закрылось, чат комнаты): Esc - убрать фокус; поле не
      // в плеере (от прошлой страницы) - Esc работает как обычно.
      if (e.code !== 'Escape') return;
      e.target.blur();
      if (box?.contains(e.target)) return;
    }
    const k = e.code;
    if(e.repeat && ['Space','KeyK','KeyF','KeyM','KeyN','KeyC','KeyP'].includes(k)) {e.preventDefault();return;}
    if (k === 'Space' || k === 'KeyK') { say(paused ? 'Воспроизведение' : 'Пауза'); toggle(); }
    else if (k === 'ArrowRight' || k === 'KeyL') { seek(time + 10); say('Вперёд на 10 секунд'); }
    else if (k === 'ArrowLeft' || k === 'KeyJ') { seek(time - 10); say('Назад на 10 секунд'); }
    else if (k === 'ArrowUp') setVolume(volume + 0.1);
    else if (k === 'ArrowDown') setVolume(volume - 0.1);
    else if (k === 'KeyF') fullscreen();
    else if (k === 'KeyM') setVolume(muted ? lastVolume : 0);
    else if (k === 'KeyS' && inIntro && intro) seek(intro[1]);
    else if (k === 'KeyN' && canNext) goNext();
    else if (k === 'KeyC' && info?.play.subs.length) pickSubs(subId ? '' : info.play.subs[0].id);
    else if (k === 'Escape') {
      // Esc не выводит окно из полноэкранного режима: закрывает меню, иначе - назад в сервис.
      if (panel) panel = '';
      else if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
      else back();
    }
    else if (k === 'Period' && e.shiftKey) setRate(rate + 0.25);
    else if (k === 'Comma' && e.shiftKey) setRate(rate - 0.25);
    else if (k === 'KeyP' && pipOk) pip();
    else if (e.key === '?') panel = panel === 'keys' ? '' : 'keys';
    else if (/^Digit\d$/.test(k) && duration) { const target = (duration * Number(k.slice(5))) / 10; seek(target); say(`Переход к ${clock(target)}`); }
    else return;
    poke();
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

<svelte:window onkeydown={key} onpointermove={poke} />

<div class="player" class:idle={!shown} bind:this={box} style:--top-h="{topH}px" style:--bottom-h="{bottomH}px" onpointermove={poke} role="application" aria-label="Видеоплеер">
  <!-- svelte-ignore a11y_media_has_caption -->
  <video
    bind:this={video}
    playsinline
    preload="auto"
    onloadedmetadata={onLoaded}
    ondurationchange={() => (mediaDuration = video && isFinite(video.duration) ? video.duration : 0)}
    ontimeupdate={onTime}
    onprogress={onTime}
    onplay={() => {
      paused = false; desiredPlaying = true;
      hls?.resumeBuffering();
      poke();
    }}
    onpause={() => {
      paused = true;
      if(!switching)desiredPlaying = false;
      hls?.pauseBuffering();
      waiting = false;
      shown = true;
      save(true);
    }}
    onwaiting={onWaiting}
    onplaying={() => (waiting = false)}
    oncanplay={() => {waiting = false;if(!desiredPlaying)hls?.pauseBuffering();}}
    onended={onEnded}
    onerror={onError}
    onpointerup={tap}
    ondblclick={(e) => (e as MouseEvent).detail && matchMedia('(pointer: fine)').matches && fullscreen()}
  ></video>

  {#if jump}
    {#key jump.key}
      <div class="jump {jump.side}" aria-hidden="true">
        <Icon name={jump.side === 'right' ? 'forward' : 'rewind'} size={30} />
        <b>{jump.side === 'right' ? '+' : '−'}{jump.n} с</b>
      </div>
    {/key}
  {/if}

  {#if subtitle}
    <div class="subs {subLook.look} size-{subLook.size}" class:up={shown} class:high={subLook.high}>{subtitle}</div>
  {/if}

  {#if waiting && !error}
    <div class="spinner" aria-label="Загрузка"></div>
  {/if}

  {#if toast}<div class="toast" role="status">{toast}</div>{/if}
  {#if mismatch}
    <div class="elsewhere" role="status">
      <span>Похоже, это другое видео: <b>{clock(duration)}</b> вместо <b>{clock(expect ?? 0)}</b></span>
      {#if onwrong}<button class="go" onclick={() => { mismatchOff = true; onwrong?.(); }}>Не тот фильм</button>{/if}
      <button class="x" aria-label="Это он, смотреть" onclick={() => (mismatchOff = true)}><Icon name="close" size={16} /></button>
    </div>
  {/if}
  {#if elsewhere !== null}
    <div class="elsewhere" role="status">
      <span>На другом устройстве вы остановились на <b>{clock(elsewhere)}</b></span>
      <button class="go" onclick={goElsewhere}>Перейти</button>
      <button class="x" aria-label="Остаться здесь" onclick={() => (elsewhere = null)}><Icon name="close" size={16} /></button>
    </div>
  {/if}
  {#if mutedByBrowser}
    <button class="unmute" onclick={unmute}><Icon name="audio" size={18} /> Браузер выключил звук - включить</button>
  {/if}

  <!-- Совместный просмотр: чат и реакции поверх видео (видны и во весь экран). -->
  {@render children?.()}

  {#if error && sync}
    <div class="room-error" role="status">{error}</div>
  {:else if error}
    <div class="err">
      <Icon name="info" size={40} />
      <p>{error}</p>
      <div class="row">
        <button class="btn" onclick={back}><Icon name="back" size={16} /> Назад</button>
        {#if info?.hls && quality === 'orig'}
          <button class="btn" onclick={() => setQuality('auto')}><Icon name="refresh" size={16} /> Адаптивное качество</button>
        {/if}
        {#if src}
          {#if onfail}<button class="btn white" onclick={onfail}><Icon name="globe" size={16} /> Плеер сайта</button>{/if}
        {:else}
          <a class="btn white" href="/api/files/{fileId}/download" download><Icon name="download" size={16} /> Скачать</a>
        {/if}
      </div>
    </div>
  {/if}

  {#if inIntro && intro}
    <button class="skip" class:low={!shown} onclick={() => seek(intro[1])}>Пропустить заставку</button>
  {/if}

  {#if (inCredits || (countdown !== null && !inCredits)) && canNext}
    <div class="nextcard" class:low={!shown}>
      {#if info?.nextInfo?.still}<img src={artW(info.nextInfo.still, 640)} alt="" />{/if}
      <div class="nc">
        <span class="muted">Следующая серия</span>
        <b>{nextSource?.title ?? info?.nextInfo?.label ?? ''}</b>
        <div class="ncb">
          <button class="btn sm" onclick={() => ((creditsSkipped = true), clearInterval(ticker), (countdown = null))}>{inCredits ? 'Смотреть титры' : 'Остаться'}</button>
          <button class="btn sm white go" onclick={goNext} style:--p="{countdown === null ? 0 : ((10 - countdown) / 10) * 100}%">
            <Icon name="play" size={13} /> Смотреть{countdown !== null ? ` · ${countdown}` : ''}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <div class="top" bind:clientHeight={topH}>
    <button class="btn ghost icon" aria-label="Назад" onclick={back}><Icon name="back" size={24} /></button>
    {#if info}
      <div class="name">
        <b>{info.title}</b>
        {#if info.episode}<span>{info.episode}</span>{/if}
      </div>
    {/if}
    <span class="grow"></span>
    {#if !sync && (fileId || src?.linkId)}<button class="btn ghost icon" disabled={roomBusy} aria-label="Смотреть вместе" title="Смотреть вместе с текущего места" onclick={watchTogether}><Icon name="party" size={22}/></button>{/if}
  </div>

  {#if paused && !waiting && !error && info}
    <button class="center" aria-label="Смотреть" onclick={toggle}><Icon name="play" size={36} /></button>
  {/if}
  {#if info && !error && !waiting}
    <div class="touch-playback" class:visible={shown && !panel}>
      <button aria-label="Назад на 10 секунд" onclick={() => { seek(time - 10); poke(); }}><Icon name="rewind" size={36}/><small>10</small></button>
      <button class="touch-toggle" aria-label={paused ? 'Смотреть' : 'Пауза'} onclick={toggle}>{#key paused}<span class="swap"><Icon name={paused ? 'play' : 'pause'} size={48}/></span>{/key}</button>
      <button aria-label="Вперёд на 10 секунд" onclick={() => { seek(time + 10); poke(); }}><Icon name="forward" size={36}/><small>10</small></button>
    </div>
  {/if}

  <div class="bottom" bind:clientHeight={bottomH} role="group" aria-label="Управление просмотром" onpointerenter={() => { controlsHover = true; poke(); }} onpointerleave={() => { controlsHover = false; poke(); }} onfocusin={poke}>
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
      <button class="cbtn" aria-label={paused ? 'Смотреть' : 'Пауза'} onclick={toggle}>{#key paused}<span class="swap"><Icon name={paused ? 'play' : 'pause'} size={24} /></span>{/key}</button>
      <button class="cbtn" aria-label="Назад на 10 секунд" onclick={() => seek(time - 10)}><Icon name="rewind" size={22} /></button>
      <button class="cbtn" aria-label="Вперёд на 10 секунд" onclick={() => seek(time + 10)}><Icon name="forward" size={22} /></button>
      <div class="vol">
        <button class="cbtn" aria-label={muted ? 'Включить звук' : 'Выключить звук'} onclick={() => setVolume(muted ? lastVolume : 0)}>
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
      {#if info?.kind === 'series' && !src}<button class="cbtn" class:on={panel === 'episodes'} aria-label="Сезоны и серии" title="Сезоны и серии" onclick={openEpisodes}><Icon name="tv" size={22} /></button>{/if}
      {#if info?.play.subs.length}
        <button class="cbtn" class:on={panel === 'subs' || !!subId} aria-label="Субтитры" title="Субтитры" onclick={() => (panel = panel === 'subs' ? '' : 'subs')}>
          <Icon name="captions" size={22} />
        </button>
      {/if}
      {#if info?.hls || (src && srcLevels.length > 1)}
        <button class="cbtn q" class:on={panel === 'quality'} aria-label="Качество" title="Качество" onclick={() => (panel = panel === 'quality' ? '' : 'quality')}>
          {qualityLabel}
        </button>
      {/if}
      <button class="cbtn q" class:on={panel === 'speed' || rate !== 1} aria-label="Скорость" title="Скорость" onclick={() => (panel = panel === 'speed' ? '' : 'speed')}>
        ×{String(rate).replace('.', ',')}
      </button>
      <button class="cbtn" class:on={panel === 'settings'} aria-label="Настройки плеера" title="Настройки" onclick={() => panel = panel === 'settings' ? '' : 'settings'}><Icon name="sliders" size={22} /></button>
      {#if canNext}
        <button class="cbtn" aria-label="Следующая серия" title="Следующая серия" onclick={goNext}><Icon name="next" size={22} /></button>
      {/if}
      {#if pipOk}
        <button class="cbtn hide-sm" aria-label="Картинка в картинке" title="Картинка в картинке (P)" onclick={pip}><Icon name="copy" size={20} /></button>
      {/if}
      <button class="cbtn hide-sm" aria-label="Горячие клавиши" title="Горячие клавиши (?)" onclick={() => (panel = panel === 'keys' ? '' : 'keys')}><Icon name="info" size={20} /></button>
      <button class="cbtn" aria-label="Во весь экран" onclick={fullscreen}><Icon name={full ? 'exitfs' : 'fullscreen'} size={22} /></button>
    </div>
  </div>

  {#if panel && info}
    <div class="panel" role="menu">
      {#if panel === 'episodes'}
        <h4>Сезоны и серии</h4>
        <div class="episode-seasons">{#each [...new Set(episodeFiles.map((f) => f.season ?? 1))] as season}<button role="menuitemradio" aria-checked={episodeSeason === season} onclick={() => episodeSeason = season}>{season} сезон</button>{/each}</div>
        {#if episodesBusy}<p class="hint">Загружаем серии…</p>{/if}
        {#each episodeFiles.filter((f) => (f.season ?? 1) === episodeSeason) as file (file.id)}
          <button class="episode-option" class:on={file.id === fileId} role="menuitem" onclick={() => pickEpisode(file.id)}>{#if file.still}<img src={artW(file.still, 342)} alt="" loading="lazy" />{/if}<span><b>{file.episode}. {localized(file.name, 'Серия')}</b>{#if localized(file.overview)}<small>{localized(file.overview)}</small>{/if}<small>{file.duration ? `${Math.round(file.duration / 60)} мин` : ''}{file.progress?.position ? ` · ${clock(file.progress.position)}` : ''}</small></span></button>
        {/each}
      {:else if panel === 'settings'}
        <h4>Настройки просмотра</h4>
        <button role="menuitem" onclick={() => panel = 'quality'}><Icon name="sliders" size={18} /> Качество <small>{qualityLabel}</small></button>
        <button role="menuitem" onclick={() => panel = 'speed'}><Icon name="play" size={18} /> Скорость <small>{rate === 1 ? 'Обычная' : `×${rate}`}</small></button>
        {#if info.play.subs.length}<button role="menuitem" onclick={() => panel = 'subs'}><Icon name="captions" size={18} /> Субтитры и размер</button>{/if}
        {#if info.play.audio.length > 1}<button role="menuitem" onclick={() => panel = 'audio'}><Icon name="audio" size={18}/> Озвучка</button>{/if}
        {#if src && alts.length > 1}<button role="menuitem" onclick={() => panel = 'sources'}><Icon name="link" size={18} /> Источник видео</button>{/if}
        {#if src && onwrong}<button role="menuitem" onclick={() => { panel = ''; onwrong?.(); }}><Icon name="flag" size={18} /> Это не тот фильм</button>{/if}
        <button role="menuitem" onclick={() => panel = 'keys'}><Icon name="info" size={18} /> Горячие клавиши</button>
      {:else if panel === 'sources'}
        <h4>Источник видео</h4>
        {#if src}
          {#each alts as a (a.id)}
            <button role="menuitemradio" aria-checked={a.id === src.linkId} class:on={a.id === src.linkId} onclick={() => { panel = ''; if (a.id !== src?.linkId) onalt?.(a.id); }}>
              <Icon name="check" size={16} /> {a.label}
            </button>
          {/each}
        {/if}
      {:else if panel === 'audio'}
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
        <h4 class="sep">Вид</h4>
        <div class="looks" role="radiogroup" aria-label="Вид субтитров">
          {#each SUB_LOOKS as l (l.id)}
            <button role="radio" aria-checked={subLook.look === l.id} class="look {l.id}" class:on={subLook.look === l.id} onclick={() => setLook({ look: l.id })}>
              <span>Аа</span>{l.label}
            </button>
          {/each}
        </div>
        <div class="seg" role="radiogroup" aria-label="Размер субтитров">
          {#each SUB_SIZES as z (z.id)}
            <button role="radio" aria-checked={subLook.size === z.id} class:on={subLook.size === z.id} onclick={() => setLook({ size: z.id })}>{z.label}</button>
          {/each}
        </div>
        <div class="seg" role="radiogroup" aria-label="Положение субтитров">
          <button role="radio" aria-checked={!subLook.high} class:on={!subLook.high} onclick={() => setLook({ high: false })}>Внизу</button>
          <button role="radio" aria-checked={subLook.high} class:on={subLook.high} onclick={() => setLook({ high: true })}>Выше</button>
        </div>
      {:else if panel === 'speed'}
        <h4>Скорость</h4>
        {#each SPEEDS as r (r)}
          <button role="menuitemradio" aria-checked={rate === r} class:on={rate === r} onclick={() => setRate(r)}>
            <Icon name="check" size={16} /> {r === 1 ? 'Обычная' : `×${String(r).replace('.', ',')}`}
          </button>
        {/each}
      {:else if panel === 'keys'}
        <h4>Горячие клавиши</h4>
        <dl class="keys">
          <dt>Пробел, K</dt><dd>пауза</dd>
          <dt>← →, J L</dt><dd>±10 секунд</dd>
          <dt>1…9</dt><dd>к 10…90 % фильма</dd>
          <dt>↑ ↓</dt><dd>громкость</dd>
          <dt>M</dt><dd>без звука</dd>
          <dt>F</dt><dd>во весь экран</dd>
          <dt>Shift + &lt; &gt;</dt><dd>скорость</dd>
          <dt>C</dt><dd>субтитры</dd>
          <dt>S</dt><dd>пропустить заставку</dd>
          <dt>N</dt><dd>следующая серия</dd>
          <dt>P</dt><dd>картинка в картинке</dd>
        </dl>
        <p class="hint">На телефоне: два касания у края - ±10 секунд, одно - показать управление.</p>
      {:else if panel === 'quality' && src}
        <h4>Качество</h4>
        <button role="menuitemradio" aria-checked={srcLevel === -1} class:on={srcLevel === -1} onclick={() => pickLevel(-1)}>
          <Icon name="check" size={16} /> Авто <small>под скорость интернета</small>
        </button>
        {#each srcLevels as l (l.i)}
          <button role="menuitemradio" aria-checked={srcLevel === l.i} class:on={srcLevel === l.i} onclick={() => pickLevel(l.i)}>
            <Icon name="check" size={16} /> {l.label ?? qualityTier(l.height)}p
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
      {/if}
    </div>
  {/if}
</div>

<style>
  .episode-seasons { display: flex; gap: 4px; overflow-x: auto; }
  .panel .episode-option { display: flex; align-items: flex-start; gap: 12px; }
  .episode-option img { width: 105px; aspect-ratio: 16/9; object-fit: cover; border-radius: 6px; }
  .episode-option span { display: grid; gap: 5px; min-width: 0; }
  .episode-option small { display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; overflow: hidden; font-size: 11px; color: #bbb; }
  .room-error { position: absolute; top: 82px; left: 16px; right: 64px; max-height: 88px; overflow: auto; padding: 10px 12px; border-radius: 12px; background: #24191a; color: #ffd6d6; font-size: 12px; z-index: 5; }
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
    background: #000; border-top: 1px solid #ffffff20;
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
    overflow:hidden;white-space:nowrap;text-overflow:ellipsis;
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
    background: #fff;
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
    background: #000;
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
    background: #000;
    color: #fff; outline: 1px solid #ffffff70;
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
  .subs.high {
    bottom: 18%;
  }
  .subs.size-s {
    font-size: clamp(14px, 2vw, 24px);
  }
  .subs.size-l {
    font-size: clamp(20px, 3.4vw, 40px);
  }
  /* Варианты вида: плашка (по умолчанию), тень, обводка как в кино, жёлтые. */
  .subs.shadow,
  .subs.cinema,
  .subs.yellow {
    background: none;
    padding: 0;
  }
  .subs.shadow {
    text-shadow:
      0 2px 6px rgba(0, 0, 0, 0.95),
      0 0 18px rgba(0, 0, 0, 0.7);
  }
  .subs.cinema,
  .subs.yellow {
    font-weight: 700;
    text-shadow:
      -1.5px -1.5px 0 #000,
      1.5px -1.5px 0 #000,
      -1.5px 1.5px 0 #000,
      1.5px 1.5px 0 #000,
      0 2px 6px rgba(0, 0, 0, 0.8);
  }
  .subs.yellow {
    color: #ffd84d;
  }
  .panel h4.sep {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .looks {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    padding: 0 4px 8px;
  }
  .panel .looks button {
    display: grid;
    justify-items: center;
    gap: 2px;
    padding: 8px 4px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.06);
    font-size: 12.5px;
  }
  .looks span {
    font-size: 20px;
    font-weight: 700;
    line-height: 1.2;
  }
  .look.box span {
    padding: 0 6px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.7);
  }
  .look.shadow span {
    text-shadow: 0 2px 6px #000;
  }
  .look.cinema span,
  .look.yellow span {
    -webkit-text-stroke: 1px #000;
  }
  .look.yellow span {
    color: #ffd84d;
  }
  .panel .looks button.on {
    box-shadow: inset 0 0 0 2px #fff;
  }
  .seg {
    display: flex;
    gap: 4px;
    margin: 0 4px 6px;
    padding: 3px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.06);
  }
  .panel .seg button {
    flex: 1;
    justify-content: center;
    padding: 7px 4px;
    border-radius: 8px;
    font-size: 13px;
  }
  .panel .seg button.on {
    background: #000; outline: 1px solid #ffffff70;
    color: #fff;
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
    background: #000;
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
  .panel button[role="menuitemradio"] :global(svg) {
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
    max-width: min(420px, 100%);
    overflow-wrap: anywhere;
    margin: 0;
  }
  .err .row {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 10px;
  }
  .grow {
    flex: 1;
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
    background: #000;
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
    background: #000;
    color: #fff;
    font-weight: 700;
    font-size: 15px;
    cursor: pointer;
    animation: slidein 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .skip:hover {
    background: #000;
    color: #fff; outline: 1px solid #ffffff70;
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
    background: #000;
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
  .jump {
    position: absolute;
    top: 50%;
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 18px 22px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.45);
    pointer-events: none;
    animation: jump 0.65s ease-out both;
  }
  .jump.left {
    left: 12%;
  }
  .jump.right {
    right: 12%;
  }
  @keyframes jump {
    from {
      opacity: 0;
      transform: translateY(-50%) scale(0.8);
    }
    20% {
      opacity: 1;
      transform: translateY(-50%) scale(1);
    }
    to {
      opacity: 0;
      transform: translateY(-50%) scale(1.05);
    }
  }
  .keys {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 14px;
    margin: 0 4px 6px;
    font-size: 13px;
  }
  .keys dt {
    font-weight: 700;
    white-space: nowrap;
  }
  .keys dd {
    margin: 0;
    color: rgba(255, 255, 255, 0.7);
  }
  .touch-playback { display: none; }
  @media (pointer: coarse) {
    .center { display: none; }
    .touch-playback { display: flex; position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); align-items: center; gap: clamp(24px, 8vw, 100px); opacity: 0; pointer-events: none; transition: opacity .2s ease; }
    .touch-playback.visible { opacity: 1; pointer-events: auto; }
    .touch-playback button { display: grid; place-items: center; position: relative; width: 64px; height: 64px; background: #0003; color: white; border: 0; border-radius: 50%; }
    .touch-playback small { position: absolute; bottom: 8px; font-size: 12px; font-weight: 700; }
    .controls > .cbtn:nth-child(-n+3) { display: none; }
    .top .name { min-width: 0; }
    .top .name b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  }
  @media (prefers-reduced-motion: reduce) { .touch-playback { transition: none; } }
  @media (max-width: 640px) {
    .controls .cbtn.q { display: none; }
    .controls .cbtn[aria-label="Субтитры"], .controls .cbtn[aria-label="Следующая серия"] { display: none; }
    .controls { gap: 2px; }
    .controls .cbtn { flex: none; min-width: 36px; width: 36px; }
    .panel { left: max(12px, env(safe-area-inset-left)); right: max(12px, env(safe-area-inset-right)); min-width: 0; max-width: none; max-height: min(55svh, 480px); overflow-y: auto; }
    .panel button { min-height: 44px; }
    .hide-sm {
      display: none;
    }
  }
  .toast {
    position: absolute;
    top: calc(80px + env(safe-area-inset-top));
    left: 50%;
    transform: translateX(-50%);
    padding: 10px 18px;
    border-radius: 999px;
    background: #000;
    border: 1px solid var(--line);
    font-size: 14px;
    animation: slidein 0.3s ease;
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
  .unmute {
    position: absolute;
    z-index: 6;
    top: calc(16px + env(safe-area-inset-top));
    left: 50%;
    transform: translateX(-50%);
    display: inline-flex;
    align-items: center;
    gap: 8px;
    max-width: calc(100% - 32px);
    padding: 10px 16px;
    border: 0;
    border-radius: 999px;
    background: #000;
    color: #fff; outline: 1px solid #ffffff70;
    font: inherit;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
  }


  .elsewhere {
    position: absolute;
    z-index: 7;
    top: calc(64px + env(safe-area-inset-top));
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 10px;
    width: max-content;
    max-width: calc(100% - 32px);
    padding: 8px 8px 8px 16px;
    border-radius: 999px;
    background: rgba(18, 18, 20, 0.92);
    color: #fff;
    font-size: 14px;
    animation: page-in 0.25s ease both;
  }
  .elsewhere span {
    min-width: 0;
  }
  .elsewhere .go {
    flex: none;
    padding: 8px 14px;
    border: 0;
    border-radius: 999px;
    background: #fff;
    color: #0a0a0a;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .elsewhere .x {
    flex: none;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 0;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
    cursor: pointer;
  }
  /* Отклик управления: кнопки чуть сжимаются под пальцем, значок «пуск/пауза» сменяется пружинкой. */
  .cbtn,
  .touch-playback button {
    transition: transform 0.15s ease, background-color 0.2s ease, opacity 0.2s ease;
  }
  .cbtn:active,
  .touch-playback button:active {
    transform: scale(0.86);
  }
  .swap {
    display: grid;
    place-items: center;
    animation: swap-in 0.22s cubic-bezier(0.2, 0.9, 0.3, 1.4) both;
  }
  @keyframes swap-in {
    from {
      opacity: 0;
      transform: scale(0.6) rotate(-20deg);
    }
  }
  :global(.no-motion) .swap {
    animation: none;
  }
</style>
