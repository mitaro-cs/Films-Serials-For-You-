import { describe, expect, it } from 'vitest';
import { toneOf } from './tone';

const flat = (r: number, g: number, b: number, n = 60) => Array.from({ length: n }, () => [r, g, b]).flat();

describe('toneOf', () => {
  it('красный кадр даёт красноватый тёмный цвет', () => {
    const [r, g, b] = [1, 3, 5].map((i) => parseInt(toneOf(flat(200, 40, 40)).slice(i, i + 2), 16));
    expect(r).toBeGreaterThan(g);
    expect(r).toBeGreaterThan(b);
    expect(r).toBeLessThan(200);
  });

  it('чёрно-белый кадр - почти серый, без цветного оттенка', () => {
    const t = toneOf(flat(120, 120, 120));
    const [r, g, b] = [1, 3, 5].map((i) => parseInt(t.slice(i, i + 2), 16));
    expect(Math.max(r, g, b) - Math.min(r, g, b)).toBeLessThan(24);
  });

  it('пустые данные - спокойный тёмный', () => {
    expect(toneOf([])).toMatch(/^#[0-9a-f]{6}$/);
  });
});
