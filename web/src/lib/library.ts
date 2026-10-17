import type { Card } from './types';

// Каталог-билеты: вкладки, фильтры по статусу и подписи на карточке. Чистые функции — тесты рядом.

export type Tab = 'movies' | 'series' | 'new';
export type Status = 'all' | 'want' | 'watching' | 'watched';

export const TABS = [
  { id: 'movies', label: 'Фильмы', href: '/?tab=movies', icon: 'film' },
  { id: 'series', label: 'Сериалы', href: '/?tab=series', icon: 'tv' },
  { id: 'new', label: 'Новое', href: '/?tab=new', icon: 'sparkle' },
] as const satisfies readonly { id: Tab; label: string; href: string; icon: string }[];

export const TITLES: Record<Tab, string> = { movies: 'Фильмы', series: 'Сериалы', new: 'Новое' };

export const STATUSES: { id: Status; label: string }[] = [
  { id: 'all', label: 'Все' },
  { id: 'want', label: 'Хочу посмотреть' },
  { id: 'watching', label: 'Смотрю' },
  { id: 'watched', label: 'Просмотрено' },
];

export function tabOf(q: URLSearchParams): Tab {
  const t = q.get('tab');
  return t === 'series' || t === 'new' ? t : 'movies';
}

export function statusOf(q: URLSearchParams): Status {
  const s = q.get('status');
  return s === 'want' || s === 'watching' || s === 'watched' ? s : 'all';
}

export type Counts = Record<Status, number>;

export function counts(items: Pick<Card, 'status'>[]): Counts {
  const c: Counts = { all: items.length, want: 0, watching: 0, watched: 0 };
  for (const i of items) if (i.status) c[i.status]++;
  return c;
}

/** Поиск на месте: все слова в названии, оригинальном названии, режиссёре или годе; «ё» = «е». */
export function matches(card: Card, query: string): boolean {
  const norm = (s: string) => s.toLowerCase().replace(/ё/g, 'е');
  const words = norm(query).split(/\s+/).filter(Boolean);
  if (!words.length) return true;
  const hay = norm([card.title, card.originalTitle, card.director, card.year].filter(Boolean).join(' '));
  return words.every((w) => hay.includes(w));
}

export function filter(items: Card[], status: Status, query = ''): Card[] {
  return items.filter((c) => (status === 'all' || c.status === status) && matches(c, query));
}

const pad = (n: number) => String(n).padStart(2, '0');

/** 10.04.2025 */
export function day(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()}`;
}

/** 21:54 */
export function time(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** Длительность фильма «1:52», у сериала — число серий. */
export function length(c: Pick<Card, 'kind' | 'runtime' | 'files'>): { label: string; value: string } {
  if (c.kind === 'series') return { label: 'Серий', value: String(c.files) };
  if (!c.runtime) return { label: 'Время', value: '—' };
  return { label: 'Время', value: `${Math.floor(c.runtime / 60)}:${pad(c.runtime % 60)}` };
}

/** «осталось 23 мин» / «осталось 1 ч 5 мин»; у ещё не начатой серии — её длина не нужна. */
export function left(position: number, duration: number): string {
  if (!duration || position <= 0) return 'с начала';
  const m = Math.max(1, Math.round((duration - position) / 60));
  return m >= 60 ? `осталось ${Math.floor(m / 60)} ч ${m % 60 ? `${m % 60} мин` : ''}`.trim() : `осталось ${m} мин`;
}

export function percent(position: number, duration: number): number {
  return duration > 0 ? Math.min(100, Math.max(0, (position / duration) * 100)) : 0;
}

/** «[США] Дэмьен Шазелл» — как строка под названием на макете. */
export function credit(c: Pick<Card, 'country' | 'director'>): string {
  return [c.country && `[${c.country}]`, c.director].filter(Boolean).join(' ');
}

/** Штрихкод билета: полоски от id — у каждого свой, но всегда один и тот же. */
export function barcode(id: number, bars = 34): number[] {
  let x = (id * 2654435761) >>> 0 || 1;
  const out: number[] = [];
  for (let i = 0; i < bars; i++) {
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    x >>>= 0;
    out.push(1 + (x % 3));
  }
  return out;
}
