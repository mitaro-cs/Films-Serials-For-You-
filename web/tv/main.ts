// ТВ-интерфейс Nova (для Samsung Tizen и других телевизоров со старым браузером): вход, главная,
// фильмы, сериалы, карточка, поиск и плеер - всё управляется пультом.

import './tv.css';
import { ApiError, post } from './api';
import { ctx, forgetLibrary, go, loadMe, session, type Cleanup, type Route, type Screen } from './app';
import { homeScreen } from './home';
import { loginScreen } from './login';
import { KEYS, current, dirOf, focusKey, isBack, isTyping, move } from './nav';
import { playerScreen } from './player';
import { searchScreen } from './search';
import { titleScreen } from './title';
import { esc } from './util';

const root = document.getElementById('app') as HTMLElement;

declare const tizen: any;
// Свои кнопки пульта (воспроизведение, перемотка) в приложении Tizen - надо заказать у системы.
try {
  const names = ['MediaPlay', 'MediaPause', 'MediaPlayPause', 'MediaStop', 'MediaRewind', 'MediaFastForward'];
  for (let i = 0; i < names.length; i++) tizen.tvinputdevice.registerKey(names[i]);
} catch (e) {
  /* в обычном браузере этого нет */
}

function parse(): Route {
  const raw = location.hash.replace(/^#/, '') || '/';
  const q = raw.indexOf('?');
  const path = q >= 0 ? raw.slice(0, q) : raw;
  const parts = path.split('/').filter(Boolean);
  return { name: parts[0] || 'home', args: parts.slice(1), query: new URLSearchParams(q >= 0 ? raw.slice(q + 1) : '') };
}

const SCREENS: Record<string, Screen> = {
  home: homeScreen('home'),
  movies: homeScreen('movies'),
  series: homeScreen('series'),
  title: titleScreen,
  search: searchScreen,
  watch: playerScreen,
  login: loginScreen,
};

let cleanup: Cleanup | void;
let token = 0;
let lastHash = '';
const remembered: Record<string, string> = {};

async function show() {
  const my = ++token;
  // Запомнить, на чём стоял фокус, чтобы вернуться на то же место по «Назад».
  const was = document.activeElement as HTMLElement | null;
  if (lastHash && was && was.getAttribute) remembered[lastHash] = was.getAttribute('data-k') || '';
  if (cleanup) cleanup();
  cleanup = undefined;
  ctx.onKey = null;
  lastHash = location.hash;

  const route = parse();
  const screen = SCREENS[route.name] || SCREENS.home;
  document.body.className = 'screen-' + route.name;
  if (route.name !== 'login' && !session.me && !(await loadMe())) {
    go('/login?next=' + encodeURIComponent(location.hash.replace(/^#/, '') || '/'), true);
    return;
  }
  window.scrollTo(0, 0);
  root.innerHTML = '<div class="loading"><i></i></div>';
  try {
    const result = await screen(root, route);
    if (my !== token) {
      if (result) result();
      return;
    }
    cleanup = result;
    focusKey(remembered[location.hash]);
  } catch (e) {
    if (my !== token) return;
    if (e instanceof ApiError && e.status === 401) {
      session.me = null;
      go('/login?next=' + encodeURIComponent(location.hash.replace(/^#/, '') || '/'), true);
      return;
    }
    const text = e instanceof ApiError ? e.message : 'Не удалось загрузить';
    root.innerHTML = '<div class="empty">' + esc(text) + '<a class="f btn pri" data-first data-k="retry" data-act="retry">Повторить</a></div>';
    focusKey('retry');
  }
}

window.addEventListener('hashchange', show);

document.addEventListener('click', (e) => {
  const el = (e.target as HTMLElement).closest('[data-href],[data-act]') as HTMLElement | null;
  if (!el) return;
  const href = el.getAttribute('data-href');
  const act = el.getAttribute('data-act');
  if (href) {
    go(href);
  } else if (act === 'retry') {
    show();
  } else if (act === 'logout') {
    post('/api/auth/logout').then(
      () => {
        session.me = null;
        forgetLibrary();
        go('/login', true);
      },
      () => {},
    );
  }
});

document.addEventListener('keydown', (e) => {
  if (ctx.onKey && ctx.onKey(e)) {
    e.preventDefault();
    return;
  }
  const c = e.keyCode;
  const dir = dirOf(c);
  if (isTyping()) {
    // В поле: влево, вправо и стирание - полю; вверх и вниз - к соседним элементам; «Назад» - выйти из поля.
    if (dir === 'up' || dir === 'down') {
      e.preventDefault();
      move(dir);
    } else if (isBack(e)) {
      e.preventDefault();
      (document.activeElement as HTMLElement).blur();
    }
    return;
  }
  if (dir) {
    e.preventDefault();
    move(dir);
  } else if (c === KEYS.enter) {
    const el = current();
    if (el) {
      e.preventDefault();
      el.click();
    }
  } else if (isBack(e)) {
    e.preventDefault();
    if (location.hash.replace(/^#/, '') && location.hash !== '#/' && history.length > 1) history.back();
    else
      try {
        tizen.application.getCurrentApplication().exit();
      } catch (err) {
        /* в браузере выходить некуда */
      }
  }
});

show();
