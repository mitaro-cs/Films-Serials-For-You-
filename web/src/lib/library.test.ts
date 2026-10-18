import { describe, expect, it } from 'vitest';
import { caption, counts, credit, day, featured, filter, left, length, matches, ofTab, percent, shelves, statusOf, tabOf, time } from './library';
import type { Card } from './types';

const card = (p: Partial<Card>): Card =>
  ({ id: 1, kind: 'movie', title: 'Ла-Ла Ленд', originalTitle: 'La La Land', year: 2016, director: 'Дэмьен Шазелл', status: null, ...p }) as Card;

describe('library', () => {
  it('reads tab and status from the address', () => {
    expect(tabOf(new URLSearchParams(''))).toBe('home');
    expect(tabOf(new URLSearchParams('tab=series'))).toBe('series');
    expect(tabOf(new URLSearchParams('tab=new'))).toBe('home');
    expect(tabOf(new URLSearchParams('tab=../x'))).toBe('home');
    expect(statusOf(new URLSearchParams('status=watched'))).toBe('watched');
    expect(statusOf(new URLSearchParams('status=bad'))).toBe('all');
  });

  it('counts and filters by status and query', () => {
    const items = [card({ id: 1, status: 'want' }), card({ id: 2, status: 'watched', title: 'Ёлки' }), card({ id: 3 })];
    expect(counts(items)).toEqual({ all: 3, want: 1, watching: 0, watched: 1 });
    expect(filter(items, 'want').map((c) => c.id)).toEqual([1]);
    expect(filter(items, 'all', 'елки').map((c) => c.id)).toEqual([2]);
    expect(matches(card({}), 'la 2016')).toBe(true);
    expect(matches(card({}), 'шазелл ла')).toBe(true);
    expect(matches(card({}), 'матрица')).toBe(false);
    expect(matches(card({}), '   ')).toBe(true);
  });

  it('formats ticket lines', () => {
    const ms = new Date(2025, 3, 10, 9, 5).getTime();
    expect(day(ms)).toBe('10.04.2025');
    expect(time(ms)).toBe('09:05');
    expect(length({ kind: 'movie', runtime: 112, files: 1 })).toEqual({ label: 'Время', value: '1:52' });
    expect(length({ kind: 'movie', runtime: null, files: 1 }).value).toBe('—');
    expect(length({ kind: 'series', runtime: 45, files: 16 })).toEqual({ label: 'Серий', value: '16' });
    expect(credit({ country: 'США', director: 'Дэмьен Шазелл' })).toBe('[США] Дэмьен Шазелл');
    expect(credit({ country: null, director: null })).toBe('');
  });

  it('shows where the viewer stopped', () => {
    expect(left(0, 3000)).toBe('с начала');
    expect(left(1600, 3000)).toBe('осталось 23 мин');
    expect(left(100, 4000)).toBe('осталось 1 ч 5 мин');
    expect(left(2999, 3000)).toBe('осталось 1 мин');
    expect(percent(50, 200)).toBe(25);
    expect(percent(5, 0)).toBe(0);
    expect(percent(300, 200)).toBe(100);
  });

  it('builds the hero and shelves like Apple TV', () => {
    const now = Date.UTC(2026, 9, 1);
    const day = 86_400_000;
    const items = [
      card({ id: 1, addedAt: now - day, backdrop: '/b1', genres: ['Драма'], rating: 7 }),
      card({ id: 2, addedAt: now - 2 * day, backdrop: '/b2', status: 'watched', watchedAt: now, genres: ['Драма'] }),
      card({ id: 3, kind: 'series', addedAt: now - 3 * day, backdrop: null, poster: '/p3', genres: ['Драма'] }),
      card({ id: 4, addedAt: now - 90 * day, backdrop: '/b4', status: 'want', genres: ['Драма'], rating: 9 }),
      card({
        id: 5,
        kind: 'series',
        addedAt: now - 5 * day,
        status: 'watching',
        resume: { fileId: 9, position: 60, duration: 600, label: 'S1 E2' } as Card['resume'],
        genres: [],
      }),
    ];
    // С кадром, без просмотренного, новые первыми.
    expect(featured(items).map((c) => c.id)).toEqual([1, 4]);
    expect(ofTab(items, 'series').map((c) => c.id)).toEqual([3, 5]);
    const s = shelves(items, [5, 1], 'home', now);
    expect(s.map((x) => x.key)).toEqual(['continue', 'new', 'want', 'movies', 'series', 'genre:Драма', 'watched']);
    expect(s[0].items.map((c) => c.id)).toEqual([5]);
    expect(s.find((x) => x.key === 'movies')!.items.map((c) => c.id)).toEqual([4, 1, 2]);
    expect(shelves(items, [], 'movies', now).some((x) => x.key === 'movies')).toBe(false);
    expect(caption(items[4], now)).toBe('S1 E2');
    expect(caption(items[1], now)).toBe('Просмотрено');
    expect(caption(items[0], now)).toBe('Недавно добавлено');
    expect(caption(card({ addedAt: now - 60 * day, genres: ['Комедия'], year: 2020 }), now)).toBe('Комедия · 2020');
  });
});
