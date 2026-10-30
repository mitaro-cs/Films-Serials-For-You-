import { describe, expect, it } from 'vitest';
import type { ExtCard } from './types';
import { like, recommend, score, type Taste } from './recommend';

const f = (id: string, genres: string[], rating: number | null = 7, year: number | null = 2020) => ({ id, genres, rating, year }) as unknown as ExtCard;
const same = (g: string) => g;

describe('recommend', () => {
  it('без совпадения по жанрам - не рекомендуем', () => {
    const taste: Taste = new Map([['Драма', 3]]);
    expect(score(f('a', ['Комедия']), taste, same)).toBe(0);
  });

  it('сильнее совпадение и оценка - выше', () => {
    const taste: Taste = new Map();
    like(taste, ['Драма'], 3);
    like(taste, ['Фантастика'], 2);
    const out = recommend([f('a', ['Драма'], 6), f('b', ['Драма', 'Фантастика'], 8.5), f('c', ['Фантастика'], 7)], taste, () => false, same);
    expect(out.map((x) => x.id)).toEqual(['b', 'a', 'c']);
  });

  it('пропускает отмеченное и не набирает один жанр подряд', () => {
    const taste: Taste = new Map([['Боевик', 3]]);
    const many = Array.from({ length: 12 }, (_, i) => f(`x${i}`, ['Боевик'], 8));
    const out = recommend(many, taste, (c) => c.id === 'x0', same, 20, 4);
    expect(out).toHaveLength(4);
    expect(out.some((c) => c.id === 'x0')).toBe(false);
  });
});
