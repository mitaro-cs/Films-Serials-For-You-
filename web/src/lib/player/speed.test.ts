import { describe, expect, it } from 'vitest';
import { abrEstimate, startQuality } from './speed';

describe('качество на старте', () => {
  it('«Оригинал» только с запасом в полтора раза', () => {
    expect(startQuality(9, 5.5)).toBe('orig');
    expect(startQuality(8, 5.5)).toBe('auto');
    expect(startQuality(3, 5.5)).toBe('auto');
  });
  it('без замера или битрейта - как раньше, «Оригинал»', () => {
    expect(startQuality(null, 5.5)).toBe('orig');
    expect(startQuality(3, null)).toBe('orig');
  });
  it('оценка для автокачества - с запасом и не ниже 300 кбит/с', () => {
    expect(abrEstimate(4)).toBe(3_000_000);
    expect(abrEstimate(0.1)).toBe(300_000);
    expect(abrEstimate(null)).toBeUndefined();
  });
});
