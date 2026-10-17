import { put } from './api';
import type { Card } from './types';

// Действия с карточкой отовсюду (ряд, всплывающая карточка, страница фильма): меняем сам
// объект — все места, где он показан, обновятся сразу; сервер — следом.

type Mark = Pick<Card, 'id' | 'inList' | 'status' | 'watchedAt' | 'resume'>;

/** «Хочу посмотреть» — это «Мой список»; статус «Смотрю» и «Просмотрено» главнее. */
export async function toggleList(card: Pick<Card, 'id' | 'inList'> & Partial<Pick<Card, 'status'>>) {
  const before = { inList: card.inList, status: card.status };
  card.inList = !card.inList;
  if (card.status === undefined || card.status === null || card.status === 'want') card.status = card.inList ? 'want' : null;
  try {
    await put(`/api/list/${card.id}`, { value: card.inList });
  } catch {
    Object.assign(card, before);
  }
}

/** «Просмотрено» — белый билет с датой; снять — и прогресс с начала. */
export async function toggleWatched(card: Mark) {
  const before = { inList: card.inList, status: card.status, watchedAt: card.watchedAt, resume: card.resume };
  const on = card.status !== 'watched';
  if (on) Object.assign(card, { status: 'watched', watchedAt: Date.now(), inList: false, resume: null });
  else Object.assign(card, { status: null, watchedAt: null, resume: null });
  try {
    await put(`/api/titles/${card.id}/watched`, { value: on });
  } catch {
    Object.assign(card, before);
  }
}

/** Меню статуса у карточки — одно на страницу (как и всплывающая карточка). */
export const statusMenu = $state<{ card: Card | null; rect: DOMRect | null }>({ card: null, rect: null });

export function openStatus(card: Card, el: HTMLElement) {
  statusMenu.card = card;
  statusMenu.rect = el.getBoundingClientRect();
}

export async function rate(card: Pick<Card, 'id' | 'myRating'>, value: -1 | 1 | 2) {
  const before = card.myRating;
  const next = before === value ? null : value;
  card.myRating = next;
  try {
    await put(`/api/titles/${card.id}/rating`, { value: next ?? 0 });
  } catch {
    card.myRating = before;
  }
}

/** Всплывающая карточка при наведении (как у Netflix): над постером, крупнее. */
export const hover = $state<{ card: Card | null; rect: DOMRect | null }>({ card: null, rect: null });

let timer: ReturnType<typeof setTimeout> | undefined;
let leaveTimer: ReturnType<typeof setTimeout> | undefined;

/** Только мышь: на сенсорных экранах карточка мешала бы прокрутке. */
export const canHover = () => typeof matchMedia !== 'undefined' && matchMedia('(hover: hover) and (pointer: fine)').matches;

export function hoverEnter(card: Card, el: HTMLElement) {
  if (!canHover()) return;
  clearTimeout(timer);
  clearTimeout(leaveTimer);
  timer = setTimeout(() => {
    hover.card = card;
    hover.rect = el.getBoundingClientRect();
  }, 420);
}

export function hoverLeave() {
  clearTimeout(timer);
  leaveTimer = setTimeout(() => {
    hover.card = null;
  }, 120);
}

export function hoverKeep() {
  clearTimeout(leaveTimer);
}
