import { put } from './api';
import type { Card } from './types';

// Действия с карточкой отовсюду (ряд, всплывающая карточка, страница фильма): меняем сам
// объект — все места, где он показан, обновятся сразу; сервер — следом.

export async function toggleList(card: Pick<Card, 'id' | 'inList'>) {
  card.inList = !card.inList;
  try {
    await put(`/api/list/${card.id}`, { value: card.inList });
  } catch {
    card.inList = !card.inList;
  }
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
