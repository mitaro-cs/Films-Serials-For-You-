import { errorText, put } from './api';
import { extChanged } from './discover/sheet.svelte';
import type { Card, ExtCard } from './types';
import { toast } from './ui/toast.svelte';
import { catalog } from './catalog.svelte';

// Статусы одним нажатием - с постера, из «Продолжить», со страницы фильма: «Хочу», «Смотрю»,
// «Просмотрено», «Больше не смотрю». Меняем сам объект (все места, где он показан, обновятся),
// сервер - следом; у фильма с сервиса ответ - точная карточка (статус мог остаться «Смотрю»).

export type Mark = 'want' | 'watching' | 'watched';

type Local = Pick<Card, 'id' | 'status' | 'inList' | 'watchedAt' | 'resume'>;

export async function setStatus(card: Local, next: Mark | 'dropped' | null) {
  const before = { status: card.status, inList: card.inList, watchedAt: card.watchedAt, resume: card.resume };
  card.status = next;
  card.inList = next === 'want';
  if (next === 'watched') card.watchedAt = Date.now();
  try {
    const c = await put<Card>(`/api/titles/${card.id}/status`, { status: next });
    Object.assign(card, { status: c.status, inList: c.inList, watchedAt: c.watchedAt, resume: c.resume });
    const cached = catalog.items?.find((item) => item.id === card.id);
    if (cached) Object.assign(cached, c);
    extChanged();
  } catch (x) {
    Object.assign(card, before);
    toast(errorText(x));
  }
}

export async function setMark(card: Pick<ExtCard, 'id' | 'mark'>, next: Mark | 'dropped' | null) {
  const before = card.mark;
  card.mark = next;
  try {
    await put(`/api/discover/title/${card.id}/mark`, { status: next });
    extChanged();
  } catch (x) {
    card.mark = before;
    toast(errorText(x));
  }
}

/** Нажали отметку, которая уже стоит, - снять; другую - поставить. */
export function toggle(card: Local, mark: Mark) {
  if (card.status === mark) return setStatus(card, mark === 'watching' ? 'dropped' : null);
  return setStatus(card, mark);
}

export function toggleMark(card: Pick<ExtCard, 'id' | 'mark'>, mark: Mark) {
  return setMark(card, card.mark === mark ? (mark === 'watching' ? 'dropped' : null) : mark);
}

/** «Больше не смотрю»: уходит из «Продолжить» и «Смотрю», место в фильме остаётся. С отменой. */
export async function drop(card: Local, title: string) {
  await setStatus(card, 'dropped');
  toast(`«${title}» - больше не смотрю`, 6000, { label: 'Вернуть', run: () => setStatus(card, 'watching') });
}
