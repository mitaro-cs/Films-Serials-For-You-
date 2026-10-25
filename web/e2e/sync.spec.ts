import { expect } from '@playwright/test';
import { test } from './util-test';
import { env, url } from './util';

// Смотрели на ноутбуке - открытая на ПК главная сама показывает фильм в «Продолжить просмотр».
test('место просмотра - между устройствами', async ({ page, browser }) => {
  const id = env().ids['Без описания'];
  await page.goto(url('/'));
  const row = page.getByRole('region', { name: 'Продолжить просмотр' });
  await expect(page.getByRole('link', { name: /Без описания/ }).first()).toBeAttached();

  // «Ноутбук» - другое окно того же человека - сохраняет место в фильме.
  const laptop = await (await browser.newContext({ storageState: 'e2e/.state.json' })).newPage();
  const title = await (await laptop.request.get(url(`/api/titles/${id}`))).json();
  const file = title.next?.fileId ?? title.files[0].id;
  const headers = { 'x-fsfy': '1' };
  const saved = await laptop.request.put(url(`/api/progress/${file}`), { headers, data: { position: 2, duration: 6 } });
  expect(saved.ok()).toBeTruthy();

  // Вернулись к «ПК»: окно получило фокус - главная перечитывает «Продолжить».
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await expect(row.getByRole('link', { name: /Без описания/ }).first()).toBeVisible({ timeout: 10_000 });

  await laptop.request.delete(url(`/api/progress/${file}`), { headers });
  await laptop.context().close();
});
