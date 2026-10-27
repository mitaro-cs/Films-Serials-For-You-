import { describe, expect, it } from 'vitest';
import { levels, trickFrame } from './levels';

describe('плеер', () => {
  it('ступени качества как на сервере', () => {
    expect(levels(2160)).toEqual([2160, 1440, 1080, 720, 480, 360]);
    expect(levels(1600)).toEqual([1440, 1080, 720, 480, 360]);
    expect(levels(720)).toEqual([720, 480, 360]);
    expect(levels(241)).toEqual([240]);
  });
  it('кадр перемотки', () => {
    const t = { interval: 10, width: 240, height: 136, cols: 10, rows: 10, count: 250, sheets: 3 };
    expect(trickFrame(t, 0)).toEqual({ sheet: 0, x: 0, y: 0 });
    expect(trickFrame(t, 125)).toEqual({ sheet: 0, x: 480, y: 136 });
    expect(trickFrame(t, 1005)).toEqual({ sheet: 1, x: 0, y: 0 });
    expect(trickFrame(t, 99999).sheet).toBe(2);
  });
});
