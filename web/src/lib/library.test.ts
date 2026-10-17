import { describe, expect, it } from 'vitest';
import { barcode, counts, credit, day, filter, left, length, matches, percent, statusOf, tabOf, time } from './library';
import type { Card } from './types';

const card = (p: Partial<Card>): Card =>
  ({ id: 1, kind: 'movie', title: 'Ла-Ла Ленд', originalTitle: 'La La Land', year: 2016, director: 'Дэмьен Шазелл', status: null, ...p }) as Card;

describe('library', () => {
  it('reads tab and status from the address', () => {
    expect(tabOf(new URLSearchParams(''))).toBe('movies');
    expect(tabOf(new URLSearchParams('tab=series'))).toBe('series');
    expect(tabOf(new URLSearchParams('tab=../x'))).toBe('movies');
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

  it('draws a stable barcode per title', () => {
    expect(barcode(7)).toEqual(barcode(7));
    expect(barcode(7)).not.toEqual(barcode(8));
    expect(barcode(0).every((b) => b >= 1 && b <= 3)).toBe(true);
  });
});
