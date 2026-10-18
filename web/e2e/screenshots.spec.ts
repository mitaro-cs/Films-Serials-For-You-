import { test } from './util-test';
import { env, url } from './util';

// Картинки для README из демо-данных: `make screenshots`. В обычном прогоне пропускаются.

const OUT = new URL('../../docs/screenshots/', import.meta.url).pathname;

test('скриншоты для README', async ({ page }, info) => {
  test.skip(!process.env.SCREENSHOTS, 'только по make screenshots');
  const shot = async (path: string, name: string) => {
    await page.goto(url(path));
    await page.waitForLoadState('networkidle');
    await page.screenshot({ path: `${OUT}${name}.png` });
  };
  if (info.project.name === 'desktop') {
    await shot('/', '1-home');
    await shot(`/title/${env().ids['Ла-Ла Ленд']}`, '3-movie');
    await shot('/admin?tab=access', '6-admin');
  }
  if (info.project.name === 'phone') await shot('/', '9-mobile');
});
