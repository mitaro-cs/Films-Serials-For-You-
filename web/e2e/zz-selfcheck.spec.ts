import { expect, test } from './util-test';
import { expectClean, url } from './util';

test('самопроверка: наложение ловится', async ({ page }) => {
  await page.goto(url('/'));
  await page.waitForSelector('.ticket');
  await page.evaluate(() => {
    const h = document.querySelector('h1')!;
    h.style.marginBottom = '-60px';
  });
  await expect(expectClean(page, 'проба')).rejects.toThrow(/пересекаются/);
  await page.evaluate(() => {
    document.querySelector('h1')!.style.marginBottom = '';
    const d = document.createElement('div');
    d.style.cssText = 'width:2000px;height:10px';
    d.textContent = 'x';
    document.querySelector('.page')!.append(d);
  });
  await expect(expectClean(page, 'проба')).rejects.toThrow(/шире экрана/);
});

test('скриншоты', async ({ page }, info) => {
  const ids = (await import('./util')).env().ids;
  for (const [p, n] of [['/', 'home'], ['/?status=watched', 'watched'], ['/?tab=series', 'series'], [`/title/${ids['Ла-Ла Ленд']}`, 'title'], [`/title/${ids['Тьма']}`, 'series-title'], ['/profiles', 'profiles'], ['/admin?tab=access', 'access'], ['/search?q=на', 'search']]) {
    await page.goto(url(p));
    await page.waitForLoadState('networkidle');
    await page.screenshot({ path: `/tmp/claude-0/-home-user/bb37a107-9e5f-5464-b141-88f6bc4848ef/scratchpad/${info.project.name}-${n}.png` });
  }
});
