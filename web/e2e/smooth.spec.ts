import { expect, test } from './util-test';
import { env, url } from './util';

// «Ничего не дёргается»: сдвиг вёрстки (CLS) при загрузке главных страниц почти нулевой.
// Считаются все сдвиги без недавнего ввода — как в Web Vitals. Хорошо по Google — до 0,1;
// у нас порог строже.

const LIMIT = 0.02;

for (const [path, what] of [
  ['/', 'главная'],
  ['/?tab=movies&status=watched', 'просмотрено'],
  ['/lists', 'смотрим вместе'],
  ['TITLE', 'страница фильма'],
  ['/people', 'люди'],
  ['CHAT', 'переписка'],
] as const) {
  test(`без сдвигов: ${what}`, async ({ page }) => {
    const p = path === 'TITLE' ? `/title/${env().ids['Ла-Ла Ленд']}` : path === 'CHAT' ? `/chats/${env().people.anna}` : path;
    await page.addInitScript(() => {
      (window as unknown as { __cls: number }).__cls = 0;
      new PerformanceObserver((list) => {
        for (const e of list.getEntries() as unknown as { value: number; hadRecentInput: boolean }[]) {
          if (!e.hadRecentInput) (window as unknown as { __cls: number }).__cls += e.value;
        }
      }).observe({ type: 'layout-shift', buffered: true });
    });
    await page.goto(url(p));
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(800);
    const cls = await page.evaluate(() => (window as unknown as { __cls: number }).__cls);
    console.log(`CLS ${what}: ${cls.toFixed(4)}`);
    expect(cls).toBeLessThan(LIMIT);
  });
}
