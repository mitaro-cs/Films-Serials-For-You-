// Плеер для пульта: OK - пауза, стрелки влево и вправо - перемотка (дольше держим - крупнее шаг),
// вверх - звук и субтитры, «Назад» - к фильму. Видео - `raw` (файл как есть) или `remux` (поток
// перепаковки хоста, перемотка за пределы буфера - новым потоком), как у Chromium на Windows.

import { api } from './api';
import type { PlayInfo } from '../src/lib/types';
import { hevcSupport } from '../src/lib/player/codecs';
import { cueAt, parseVtt, type Cue } from '../src/lib/player/vtt';
import { back, ctx, forgetLibrary, go, type Screen } from './app';
import { KEYS, dirOf, focusKey, isBack, move } from './nav';
import { clock, esc } from './util';

export const playerScreen: Screen = async (root, route) => {
  const fileId = Number(route.args[0]);
  const info = await api<PlayInfo>('/api/files/' + fileId + '/info');
  const p = info.play;

  root.innerHTML =
    '<div class="player"><video id="v" playsinline></video><div class="subs" id="subs"></div>' +
    '<div class="osd" id="osd"><div class="top"><b>' + esc(info.title) + '</b><span>' + esc(info.episode) + '</span></div>' +
    '<div class="low"><div class="seekbar"><i class="buf" id="buf"></i><i class="pos" id="pos"></i></div>' +
    '<div class="times"><span id="cur">0:00</span><span id="state"></span><span id="dur"></span></div>' +
    '<div class="tips">▲ звук и субтитры  ·  ◄ ► перемотка  ·  OK пауза</div></div></div>' +
    '<div class="spin off" id="spin"><i></i></div><div class="msg off" id="msg"></div>' +
    '<div class="pm off" id="pm"></div></div>';

  const $ = (id: string) => root.querySelector('#' + id) as HTMLElement;
  const v = $('v') as HTMLVideoElement;
  const osd = $('osd');
  const spin = $('spin');
  const msg = $('msg');
  const pm = $('pm');

  const hevc = hevcSupport((t) => v.canPlayType(t));
  const hevcOk = p.tenBit ? hevc.main10 : hevc.main;
  const total = () => p.duration || (isFinite(v.duration) ? v.duration : 0);

  let audio = Math.max(0, p.audio.findIndex((a) => a.default));
  let subId = '';
  let cues: Cue[] = [];
  let mode: 'raw' | 'remux' = 'raw';
  let offset = 0;
  let pendingSeek = 0;
  let forceRemux = false;
  let lastSaved = 0;
  let alive = true;

  const pos = () => offset + (v.currentTime || 0);

  function remuxUrl(t: number): string {
    let q = 't=' + t.toFixed(2) + '&audio=' + audio;
    if (hevcOk) q += '&hevc=1';
    return '/api/files/' + fileId + '/play?' + q;
  }

  function start(at: number) {
    const direct = (p.direct && audio === 0 && (!p.hevc || hevcOk)) || !p.remux;
    mode = direct && !forceRemux ? 'raw' : 'remux';
    spin.classList.remove('off');
    if (mode === 'raw') {
      offset = 0;
      pendingSeek = at;
      v.src = '/api/files/' + fileId + '/raw';
    } else {
      offset = at;
      pendingSeek = 0;
      v.src = remuxUrl(at);
    }
    const r = v.play();
    if (r && r.catch) r.catch(() => {});
  }

  // ---------- панель кадра ----------

  let hideTimer = 0;
  let panelOpen = false;
  function showOsd() {
    osd.classList.remove('off');
    window.clearTimeout(hideTimer);
    if (!v.paused && !panelOpen) hideTimer = window.setTimeout(() => osd.classList.add('off'), 4500);
  }

  let shownAt = -1;
  function paint(at: number) {
    // Экран уже закрыт (пауза от самого выхода): рисовать некуда.
    if (!alive) return;
    const d = total();
    const w = d ? Math.min(100, (at / d) * 100) : 0;
    ($('pos') as HTMLElement).style.width = w + '%';
    $('cur').textContent = clock(at);
    $('dur').textContent = d ? clock(d) : '';
    if (v.buffered.length) {
      const end = offset + v.buffered.end(v.buffered.length - 1);
      ($('buf') as HTMLElement).style.width = d ? Math.min(100, (end / d) * 100) + '%' : '0';
    }
    $('state').textContent = v.paused ? 'Пауза' : '';
    const sec = Math.floor(at);
    if (sec !== shownAt) {
      shownAt = sec;
      $('subs').textContent = subId ? cueAt(cues, at) : '';
    }
  }

  // ---------- прогресс ----------

  function save(force?: boolean) {
    const d = total();
    const at = pos();
    if (!d || at < 1) return;
    const now = Date.now();
    if (!force && now - lastSaved < 8000) return;
    lastSaved = now;
    // Без keepalive: Chrome 69 отказывается слать такие запросы с заголовками (место не сохранялось).
    fetch('/api/progress/' + fileId, {
      method: 'PUT',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-fsfy': '1' },
      body: JSON.stringify({ position: at, duration: d }),
    }).catch(() => {});
  }

  // ---------- перемотка ----------

  let target: number | null = null;
  let seekTimer = 0;
  let streak = 0;
  let lastNudge = 0;

  function nudge(sign: number, base: number) {
    const now = Date.now();
    streak = now - lastNudge < 700 ? streak + 1 : 0;
    lastNudge = now;
    const step = streak < 3 ? base : streak < 8 ? base * 3 : base * 6;
    const d = total();
    const t = (target == null ? pos() : target) + sign * step;
    target = Math.max(0, d ? Math.min(d - 2, t) : t);
    showOsd();
    paint(target);
    window.clearTimeout(seekTimer);
    seekTimer = window.setTimeout(applySeek, 450);
  }

  function applySeek() {
    const t = target;
    target = null;
    if (t == null) return;
    const have = v.buffered.length ? offset + v.buffered.end(v.buffered.length - 1) : 0;
    if (mode === 'raw' || (t >= offset && t <= have)) {
      v.currentTime = t - offset;
    } else {
      spin.classList.remove('off');
      offset = t;
      v.src = remuxUrl(t);
      const r = v.play();
      if (r && r.catch) r.catch(() => {});
    }
  }

  // ---------- звук и субтитры ----------

  function renderMenu() {
    let h = '';
    if (p.audio.length > 1) {
      h += '<h3>Звук</h3><div class="chips">';
      for (let i = 0; i < p.audio.length; i++) {
        h += '<a class="f chip' + (i === audio ? ' on' : '') + '" data-k="a' + i + '" data-act="audio" data-n="' + i + '">' + esc(p.audio[i].label) + '</a>';
      }
      h += '</div>';
    }
    h += '<h3>Субтитры</h3><div class="chips"><a class="f chip' + (!subId ? ' on' : '') + '" data-k="s-" data-act="subs" data-n="">Выключены</a>';
    for (let i = 0; i < p.subs.length; i++) {
      const s = p.subs[i];
      h += '<a class="f chip' + (subId === s.id ? ' on' : '') + '" data-k="s' + esc(s.id) + '" data-act="subs" data-n="' + esc(s.id) + '">' + esc(s.label) + '</a>';
    }
    h += '</div>';
    if (info.next) h += '<h3>Дальше</h3><div class="chips"><a class="f chip" data-k="next" data-act="next">Следующая серия' + (info.nextInfo ? ' · ' + esc(info.nextInfo.label) : '') + '</a></div>';
    pm.innerHTML = h;
  }

  function openMenu() {
    renderMenu();
    panelOpen = true;
    pm.classList.remove('off');
    showOsd();
    focusKey('a' + audio, pm);
  }

  function closeMenu() {
    panelOpen = false;
    pm.classList.add('off');
    (document.activeElement as HTMLElement | null)?.blur();
    showOsd();
  }

  async function pickSubs(id: string) {
    subId = id;
    cues = [];
    shownAt = -1;
    if (id) {
      try {
        const r = await fetch('/api/files/' + fileId + '/subs/' + encodeURIComponent(id), { credentials: 'same-origin' });
        if (r.ok) cues = parseVtt(await r.text());
      } catch (e) {
        subId = '';
      }
    }
    renderMenu();
    focusKey('s' + (subId || '-'), pm);
  }

  const onClick = (e: Event) => {
    const el = (e.target as HTMLElement).closest('[data-act]') as HTMLElement | null;
    if (!el) return;
    const act = el.getAttribute('data-act');
    if (act === 'subs') pickSubs(el.getAttribute('data-n') || '');
    else if (act === 'audio') {
      audio = Number(el.getAttribute('data-n'));
      const at = pos();
      closeMenu();
      start(at);
    } else if (act === 'next' && info.next) {
      save(true);
      go('/watch/' + info.next, true);
    }
  };
  root.addEventListener('click', onClick);

  // ---------- события видео ----------

  v.addEventListener('loadedmetadata', () => {
    if (pendingSeek > 0) {
      try {
        v.currentTime = pendingSeek;
      } catch (e) {
        /* начнём сначала */
      }
      pendingSeek = 0;
    }
  });
  v.addEventListener('playing', () => {
    spin.classList.add('off');
    showOsd();
  });
  v.addEventListener('waiting', () => spin.classList.remove('off'));
  v.addEventListener('pause', () => {
    if (!alive) return;
    showOsd();
    paint(target == null ? pos() : target);
    save(true);
  });
  v.addEventListener('play', () => {
    showOsd();
    paint(pos());
  });
  v.addEventListener('timeupdate', () => {
    if (target == null) paint(pos());
    save();
  });
  v.addEventListener('ended', () => {
    save(true);
    if (info.next) go('/watch/' + info.next, true);
    else back();
  });
  v.addEventListener('error', () => {
    if (!alive) return;
    if (mode === 'raw' && p.remux && !forceRemux) {
      forceRemux = true;
      start(pos());
      return;
    }
    spin.classList.add('off');
    msg.textContent = 'Не удалось воспроизвести. Нажмите «Назад» и выберите другую серию или фильм';
    msg.classList.remove('off');
  });

  // ---------- кнопки пульта ----------

  ctx.onKey = (e) => {
    const c = e.keyCode;
    if (isBack(e) || c === KEYS.stop) {
      if (panelOpen) closeMenu();
      else back();
      return true;
    }
    if (panelOpen) {
      const d = dirOf(c);
      if (d) return move(d);
      return false;
    }
    if (c === KEYS.left || c === KEYS.rewind) nudge(-1, c === KEYS.rewind ? 30 : 10);
    else if (c === KEYS.right || c === KEYS.forward) nudge(1, c === KEYS.forward ? 30 : 10);
    else if (c === KEYS.up) openMenu();
    else if (c === KEYS.down) osd.classList.add('off');
    else if (c === KEYS.enter || c === KEYS.space || c === KEYS.playPause || c === KEYS.play || c === KEYS.pause) {
      if (c === KEYS.play || (c !== KEYS.pause && v.paused)) {
        const r = v.play();
        if (r && r.catch) r.catch(() => {});
      } else v.pause();
    } else return false;
    return true;
  };

  // Куда вернуться: сохранённое место (или заданное ссылкой), но не в самый конец.
  const wanted = route.query.get('t');
  const d0 = p.duration || 0;
  let from = wanted ? Number(wanted) : info.position;
  if (!(from > 5) || (d0 && from > d0 - 20)) from = 0;
  start(from);
  showOsd();

  for (let i = 0; i < p.subs.length; i++) {
    const s = p.subs[i];
    if (s.forced && s.lang && s.lang.indexOf('ru') === 0) {
      pickSubs(s.id);
      break;
    }
  }

  return () => {
    alive = false;
    save(true);
    window.clearTimeout(hideTimer);
    window.clearTimeout(seekTimer);
    root.removeEventListener('click', onClick);
    v.pause();
    v.removeAttribute('src');
    v.load();
    ctx.onKey = null;
    forgetLibrary();
  };
};
