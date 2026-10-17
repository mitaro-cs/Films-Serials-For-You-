import { expect, test } from './util-test';
import { url } from './util';

// Скорость: главная качает мало кода (сжатым), плеер (hls.js) — только когда открыли просмотр,
// постеры — лёгкие и лениво.

const BUDGET_JS = 80 * 1024;

test('главная: код меньше бюджета, без hls.js', async ({ page }, info) => {
  test.skip(info.project.name !== 'desktop', 'один раз достаточно');
  const scripts: { url: string; size: Promise<number> }[] = [];
  page.on('requestfinished', (r) => {
    if (r.resourceType() === 'script') scripts.push({ url: r.url(), size: r.sizes().then((s) => s.responseBodySize) });
  });
  await page.goto(url('/'));
  await page.waitForLoadState('networkidle');
  const sizes = await Promise.all(scripts.map((s) => s.size));
  const total = sizes.reduce((a, b) => a + b, 0);
  console.log(`JS главной: ${(total / 1024).toFixed(1)} КБ (сжатый), файлов: ${scripts.length}`);
  expect(total, 'JS главной, байт').toBeLessThan(BUDGET_JS);
  expect(scripts.some((s) => /hls/i.test(s.url))).toBe(false);
});

test('постеры лёгкие и с ленивой загрузкой', async ({ page }) => {
  await page.goto(url('/'));
  const imgs = page.locator('article.ticket .art > img');
  await expect(imgs.first()).toBeVisible();
  const n = await imgs.count();
  for (let i = 0; i < n; i++) {
    const loading = await imgs.nth(i).getAttribute('loading');
    expect(i < 8 ? loading === 'eager' : loading === 'lazy').toBe(true);
  }
  // Лёгкий постер (342 px), а не исходный.
  const w = await imgs.first().evaluate((i: HTMLImageElement) => i.naturalWidth);
  expect(w).toBeLessThanOrEqual(342);
});
