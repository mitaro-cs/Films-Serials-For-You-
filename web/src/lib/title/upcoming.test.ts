import { describe, expect, it } from 'vitest';
import { upcomingYear } from './upcoming';
describe('даты будущих серий', () => {
  const now = new Date(2026,9,29);
  it('распознаёт будущие год и дату, не считая неизвестные даты будущими', () => {
    expect(upcomingYear('2027',now)).toBe(2027);
    expect(upcomingYear('2026-11-01',now)).toBe(2026);
    expect(upcomingYear('2026',now)).toBeNull();
    expect(upcomingYear('2026-10-29',now)).toBeNull();
    expect(upcomingYear(null,now)).toBeNull();
  });
});
