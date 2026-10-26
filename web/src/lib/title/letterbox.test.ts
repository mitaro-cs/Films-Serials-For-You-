import { describe, expect, it } from 'vitest';
import { barsOf, zoomFor } from './letterbox';

function frame(width: number, height: number, bar: number, fill = 120): Uint8ClampedArray {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y++) {
    const v = y < bar || y >= height - bar ? 0 : fill;
    for (let x = 0; x < width; x++) data.set([v, v, v, 255], (y * width + x) * 4);
  }
  return data;
}

describe('чёрные полосы', () => {
  it('находит полосы сверху и снизу', () => {
    expect(barsOf(frame(64, 36, 6), 64, 36)).toBeCloseTo(6 / 36);
  });
  it('без полос - ноль', () => {
    expect(barsOf(frame(64, 36, 0), 64, 36)).toBe(0);
  });
  it('полоса только сверху (титр) - не полоса', () => {
    const data = frame(64, 36, 0);
    for (let y = 0; y < 5; y++) for (let x = 0; x < 64; x++) data.set([0, 0, 0, 255], (y * 64 + x) * 4);
    expect(barsOf(data, 64, 36)).toBe(0);
  });
  it('увеличение', () => {
    expect(zoomFor(0)).toBe(1);
    expect(zoomFor(0.125)).toBeCloseTo(1 / 0.75);
    expect(zoomFor(0.4)).toBe(1.6);
  });
});
