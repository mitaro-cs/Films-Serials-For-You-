import { describe, expect, it } from 'vitest';
import { heightLabel, hevcSupport } from './codecs';

describe('HEVC в браузере', () => {
  it('Chrome на Windows: голое hvc1 - нет, полная строка - да', () => {
    const chrome = (t: string) => (/hvc1\.|hev1\./.test(t) ? 'probably' : '');
    expect(hevcSupport(chrome)).toEqual({ main: true, main10: true });
  });
  it('только Main (8 бит)', () => {
    const old = (t: string) => (t.includes('.1.6.') ? 'maybe' : '');
    expect(hevcSupport(old)).toEqual({ main: true, main10: false });
  });
  it('Firefox без HEVC', () => {
    expect(hevcSupport(() => '')).toEqual({ main: false, main10: false });
    expect(hevcSupport(null)).toEqual({ main: false, main10: false });
  });
  it('подписи качества', () => {
    expect(heightLabel(2160)).toBe('4K');
    expect(heightLabel(1440)).toBe('1440p');
    expect(heightLabel(720)).toBe('720p');
  });
});
