import { expect, test } from './util-test';
import { expectClean, url } from './util';

// Сама проверка вёрстки должна ловить ошибки, иначе её зелёный цвет ничего не значит.
test('самопроверка: наложение ловится', async ({ page }) => {
  await page.goto(url('/'));
  await page.waitForSelector('.wide');
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
    document.querySelector('main')!.append(d);
  });
  await expect(expectClean(page, 'проба')).rejects.toThrow(/шире экрана/);
});
