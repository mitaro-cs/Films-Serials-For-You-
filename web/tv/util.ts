// Мелочи ТВ-интерфейса: экранирование, время, подписи.

import type { Card } from '../src/lib/types';

export function esc(s: unknown): string {
  return String(s == null ? '' : s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/** 3725 -> «1:02:05», 125 -> «2:05». */
export function clock(sec: number): string {
  const s = Math.max(0, Math.floor(sec || 0));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  const two = (n: number) => (n < 10 ? '0' + n : String(n));
  return h > 0 ? h + ':' + two(m) + ':' + two(r) : m + ':' + two(r);
}

export function runtime(min: number | null): string {
  if (!min) return '';
  const h = Math.floor(min / 60);
  const m = Math.round(min % 60);
  return h > 0 ? h + ' ч ' + (m ? m + ' мин' : '') : m + ' мин';
}

/** «1 сезон», «2 сезона», «5 сезонов». */
export function seasons(n: number): string {
  const a = n % 10;
  const b = n % 100;
  const w = b >= 11 && b <= 14 ? 'сезонов' : a === 1 ? 'сезон' : a >= 2 && a <= 4 ? 'сезона' : 'сезонов';
  return n + ' ' + w;
}

/** Год, жанры, возраст, длительность, рейтинг - одной строкой. */
export function meta(c: Card): string {
  const parts: string[] = [];
  if (c.year) parts.push(String(c.year));
  if (c.genres.length) parts.push(c.genres.slice(0, 3).join(', '));
  if (c.age) parts.push(c.age);
  if (c.kind === 'series') {
    if (c.seasons) parts.push(seasons(c.seasons));
  } else if (c.runtime) parts.push(runtime(c.runtime));
  if (c.rating) parts.push('★ ' + c.rating.toFixed(1));
  return parts.join('  ·  ');
}

export function pct(position: number, duration: number): number {
  if (!duration) return 0;
  return Math.max(0, Math.min(100, Math.round((position / duration) * 100)));
}
