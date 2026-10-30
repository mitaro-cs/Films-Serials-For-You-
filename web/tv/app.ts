// Общее для экранов ТВ: переходы, обработчик клавиш экрана, библиотека, шапка.

import { api } from './api';
import { esc } from './util';
import type { Library, Me } from '../src/lib/types';

export type Cleanup = () => void;

export interface Route {
  name: string;
  args: string[];
  query: URLSearchParams;
}

export type Screen = (root: HTMLElement, route: Route) => Promise<Cleanup | void> | Cleanup | void;

/** Экран может перехватить клавишу: вернуть true - дальше не обрабатывать. */
export const ctx: { onKey: ((e: KeyboardEvent) => boolean) | null } = { onKey: null };

export function go(path: string, replace?: boolean) {
  const hash = '#' + path;
  if (replace) location.replace(location.pathname + location.search + hash);
  else location.hash = path;
}

export function back() {
  if (history.length > 1) history.back();
  else go('/', true);
}

// ---------- кто вошёл ----------

export const session: { me: Me | null } = { me: null };

export async function loadMe(): Promise<Me | null> {
  try {
    session.me = await api<Me>('/api/me');
  } catch (e) {
    session.me = null;
  }
  return session.me;
}

// ---------- библиотека: один запрос на все экраны, свежесть - 20 секунд ----------

let lib: Library | null = null;
let libAt = 0;

export async function library(force?: boolean): Promise<Library> {
  if (!lib || force || Date.now() - libAt > 20000) {
    lib = await api<Library>('/api/library');
    libAt = Date.now();
  }
  return lib;
}

export function forgetLibrary() {
  libAt = 0;
}

// ---------- шапка ----------

export function bar(active: string): string {
  const tabs: [string, string, string][] = [
    ['home', '/', 'Главная'],
    ['movies', '/movies', 'Фильмы'],
    ['series', '/series', 'Сериалы'],
    ['search', '/search', 'Поиск'],
  ];
  let h = '<header class="bar"><div class="logo">Nova</div><nav>';
  for (let i = 0; i < tabs.length; i++) {
    const t = tabs[i];
    h +=
      '<a class="f tab' + (t[0] === active ? ' on' : '') + '" data-k="tab-' + t[0] + '" data-href="' + t[1] + '">' +
      esc(t[2]) + '</a>';
  }
  h += '</nav><div class="who">';
  if (session.me) h += '<span>' + esc(session.me.user.name) + '</span>';
  h += '<a class="f tab" data-k="tab-out" data-act="logout">Выйти</a></div></header>';
  return h;
}
