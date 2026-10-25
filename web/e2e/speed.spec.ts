import { expect, test } from './util-test';
import { url } from './util';

// Скорость: главная качает мало кода (сжатым), плеер (hls.js) - только когда открыли просмотр,
// постеры - лёгкие и лениво.

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

test('карточки лёгкие и с ленивой загрузкой', async ({ page }) => {
  await page.goto(url('/'));
  const imgs = page.locator('main .tile .art > img');
  await expect(imgs.first()).toBeVisible();
  // Разом, одним снимком: по одной картинке WebKit иногда ждал элемент, который ряд уже перерисовал.
  const loading = await imgs.evaluateAll((list) => list.map((i) => i.getAttribute('loading')));
  const n = loading.length;
  const eager = loading.filter((l) => l === 'eager').length;
  expect(eager, 'сразу грузятся только первые карточки').toBeLessThanOrEqual(10);
  expect(n - eager, 'остальные - лениво').toBeGreaterThan(0);
  // Лёгкий кадр, а не исходник: для карточки 640 px хватает.
  const w = await imgs.first().evaluate((i: HTMLImageElement) => i.naturalWidth);
  expect(w).toBeLessThanOrEqual(1280);
});
