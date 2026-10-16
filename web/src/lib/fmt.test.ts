import { describe, expect, it } from 'vitest';
import { clock, count, date, runtime, size } from './fmt';

describe('fmt', () => {
  it('formats durations', () => {
    expect(runtime(128)).toBe('2 ч 8 мин');
    expect(runtime(45)).toBe('45 мин');
    expect(clock(3725)).toBe('1:02:05');
    expect(clock(65)).toBe('1:05');
  });
  it('pluralizes in Russian', () => {
    expect(count(1, 'серия', 'серии', 'серий')).toBe('1 серия');
    expect(count(3, 'серия', 'серии', 'серий')).toBe('3 серии');
    expect(count(11, 'серия', 'серии', 'серий')).toBe('11 серий');
    expect(count(25, 'серия', 'серии', 'серий')).toBe('25 серий');
  });
  it('formats sizes and dates', () => {
    expect(size(1536 * 1024 * 1024)).toBe('1,5 ГБ');
    expect(date('2016-12-25')).toBe('25 декабря 2016');
  });
});
