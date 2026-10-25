import type { Card } from './types';

// Каталог как у Apple TV: вкладки, герой, ряды, фильтры по статусу и подписи на карточке.
// Чистые функции — тесты рядом.

export type Tab = 'home' | 'movies' | 'series';
export type Status = 'all' | 'want' | 'watching' | 'watched';

export const TABS = [
  { id: 'home', label: 'Главная', href: '/' },
  { id: 'movies', label: 'Фильмы', href: '/?tab=movies' },
  { id: 'series', label: 'Сериалы', href: '/?tab=series' },
] as const satisfies readonly { id: Tab; label: string; href: string }[];

export const STATUSES: { id: Status; label: string }[] = [
  { id: 'all', label: 'Все' },
  { id: 'want', label: 'Хочу посмотреть' },
  { id: 'watching', label: 'Смотрю' },
  { id: 'watched', label: 'Просмотрено' },
];

export function tabOf(q: Pick<URLSearchParams, 'get'>): Tab {
  const t = q.get('tab');
  return t === 'movies' || t === 'series' ? t : 'home';
}

export function ofTab(items: Card[], tab: Tab): Card[] {
  if (tab === 'home') return items;
  const kind = tab === 'movies' ? 'movie' : 'series';
  return items.filter((c) => c.kind === kind);
}

const DAY = 86_400_000;

/** Герой: до 6 недавно добавленных с большим кадром (нет кадров — с постером). */
export function featured(items: Card[], n = 6): Card[] {
  const fresh = [...items].sort((a, b) => b.addedAt - a.addedAt);
  const wide = fresh.filter((c) => c.backdrop && c.status !== 'watched');
  const pick = wide.length ? wide : fresh.filter((c) => c.backdrop || c.poster);
  return pick.slice(0, n);
}

export interface Shelf {
  key: string;
  title: string;
  items: Card[];
}

/** Ряды главной: продолжить, новое, хочу, фильмы, сериалы, жанры, просмотренное. */
export function shelves(items: Card[], recent: number[], tab: Tab, now = Date.now()): Shelf[] {
  const out: Shelf[] = [];
  const add = (key: string, title: string, list: Card[]) => {
    if (list.length) out.push({ key, title, items: list.slice(0, 24) });
  };
  const byId = new Map(items.map((c) => [c.id, c]));
  const going = recent.map((id) => byId.get(id)).filter((c): c is Card => !!c && c.status === 'watching' && !!c.resume);
  add('continue', 'Продолжить просмотр', going);
  const newest = [...items].sort((a, b) => b.addedAt - a.addedAt).filter((c) => c.status !== 'watched');
  const month = newest.filter((c) => now - c.addedAt < 30 * DAY);
  add('new', 'Новое', month.length >= 4 ? month : newest);
  add('want', 'Хочу посмотреть', items.filter((c) => c.status === 'want'));
  if (tab === 'home') {
    const top = (kind: Card['kind']) =>
      items.filter((c) => c.kind === kind).sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0) || b.addedAt - a.addedAt);
    add('movies', 'Фильмы', top('movie'));
    add('series', 'Сериалы', top('series'));
  }
  const genres = new Map<string, Card[]>();
  for (const c of items) for (const g of c.genres.slice(0, 2)) genres.set(g, [...(genres.get(g) ?? []), c]);
  [...genres.entries()]
    .filter(([, l]) => l.length >= 4)
    .sort((a, b) => b[1].length - a[1].length)
    .slice(0, 4)
    .forEach(([g, l]) => add(`genre:${g}`, g, l));
  const watched = items.filter((c) => c.status === 'watched').sort((a, b) => (b.watchedAt ?? 0) - (a.watchedAt ?? 0));
  add('watched', 'Просмотрено', watched);
  return out;
}

/** Подпись под широкой карточкой: где остановился, статус, «недавно добавлено» или жанр и год. */
export function caption(c: Card, now = Date.now()): string {
  if (c.status === 'watching' && c.resume) return c.resume.label || left(c.resume.position, c.resume.duration);
  if (c.status === 'watched') return 'Просмотрено';
  if (c.status === 'want') return 'Хочу посмотреть';
  if (now - c.addedAt < 14 * DAY) return 'Недавно добавлено';
  return [c.genres[0], c.year].filter(Boolean).join(' · ') || (c.kind === 'series' ? 'Сериал' : 'Фильм');
}

export function statusOf(q: Pick<URLSearchParams, 'get'>): Status {
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
