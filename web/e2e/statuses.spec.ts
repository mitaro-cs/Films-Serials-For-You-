import { expect, test } from './util-test';
import { env, resetStatuses, url } from './util';

// Статусы с карточки: меню, «Хочу посмотреть», «Просмотрено» (белый билет), «Продолжить».
// Демо-данные общие: перед каждым тестом статусы — как после setup.

test.beforeEach(resetStatuses);

test('статус ставится и снимается с карточки, числа у фильтров меняются', async ({ page }) => {
  await page.goto(url('/'));
  const card = page.locator('article.ticket', { has: page.getByRole('link', { name: 'Амели' }) });
  const chip = (name: string) => page.getByRole('tab', { name: new RegExp(`^${name}`) });
  await expect(chip('Хочу посмотреть')).toContainText('2');

  await card.getByRole('button', { name: /Статус/ }).click();
  await page.getByRole('menuitemcheckbox', { name: /Хочу посмотреть/ }).click();
  await expect(chip('Хочу посмотреть')).toContainText('3');
  await chip('Хочу посмотреть').click();
  await expect(page).toHaveURL(/status=want/);
  await expect(page.getByRole('link', { name: 'Амели' })).toBeVisible();

  // Отметка пережила перезагрузку — сохранена на сервере.
  await page.reload();
  await expect(page.getByRole('link', { name: 'Амели' })).toBeVisible();
  await page.locator('article.ticket', { has: page.getByRole('link', { name: 'Амели' }) }).getByRole('button', { name: /Статус/ }).click();
  await page.getByRole('menuitemcheckbox', { name: /Хочу посмотреть/ }).click();
  await expect(page.getByRole('link', { name: 'Амели' })).toHaveCount(0);
});

test('«Просмотрено» — белый билет со штрихкодом и датой', async ({ page }) => {
  await page.goto(url('/'));
  const card = () => page.locator('article.ticket', { has: page.getByRole('link', { name: 'Дюна' }) });
  await card().getByRole('button', { name: /Статус/ }).click();
  await page.getByRole('menuitemcheckbox', { name: /Просмотрено/ }).click();
  await expect(card()).toHaveClass(/white/);
  await expect(card().locator('.code i').first()).toBeVisible();
  await expect(card()).toContainText(/Просмотрено\s*\d\d\.\d\d\.\d{4}/);
  // Вернуть «Хочу посмотреть», как в демо-данных.
  await card().getByRole('button', { name: /Статус/ }).click();
  await page.getByRole('menuitemcheckbox', { name: /Просмотрено/ }).click();
  await expect(card()).not.toHaveClass(/white/);
  await card().getByRole('button', { name: /Статус/ }).click();
  await page.getByRole('menuitemcheckbox', { name: /Хочу посмотреть/ }).click();
  await expect(card().getByRole('button', { name: /хочу посмотреть/ })).toBeVisible();
});

test('где остановился — видно в «Продолжить», на карточке и на странице', async ({ page }) => {
  await page.goto(url('/'));
  const go = page.getByRole('region', { name: 'Продолжить просмотр' }).getByRole('link', { name: /Начало/ });
  await expect(go).toBeVisible();
  await expect(go).toHaveAttribute('href', /\/watch\/\d+/);
  const card = page.locator('article.ticket', { has: page.getByRole('link', { name: 'Начало' }) });
  await expect(card).toContainText('осталось');
  await expect(card.locator('.bar i')).toHaveAttribute('style', /width: 4\d/);

  // Сериал: досмотрена первая серия — дальше вторая.
  await page.goto(url('/?tab=series'));
  await expect(page.locator('article.ticket', { has: page.getByRole('link', { name: 'Тьма' }) })).toContainText('1 сезон, 2 серия');
  await page.goto(url(`/title/${env().ids['Тьма']}`));
  await expect(page.locator('.where')).toContainText('1 сезон, 2 серия');
});

test('поиск на месте и вкладки', async ({ page }, info) => {
  await page.goto(url('/'));
  await page.getByRole('button', { name: 'Найти в разделе' }).click();
  await page.getByRole('searchbox', { name: 'Найти в разделе' }).fill('нолан');
  await expect(page.locator('article.ticket')).toHaveCount(2);
  await page.getByRole('searchbox', { name: 'Найти в разделе' }).fill('нет такого');
  await expect(page.getByText('Ничего не нашлось')).toBeVisible();

  // Широкий экран — капсула в шапке; уже 1000 px — капсула разделов на самой странице.
  const nav = info.project.name === 'desktop' ? page.locator('nav.pill') : page.getByRole('navigation', { name: 'Разделы каталога' });
  await nav.getByRole('link', { name: 'Сериалы' }).click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Сериалы');
  await expect(page.locator('article.ticket')).toHaveCount(1);
});

test('меню статуса целиком на экране и закрывается по Esc', async ({ page }) => {
  await page.goto(url('/'));
  const last = page.locator('article.ticket').last();
  await last.scrollIntoViewIfNeeded();
  await last.getByRole('button', { name: /Статус/ }).click();
  const menu = page.getByRole('menu');
  await expect(menu).toBeVisible();
  const box = (await menu.boundingBox())!;
  const vp = page.viewportSize()!;
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(vp.width);
  expect(box.y + box.height).toBeLessThanOrEqual(vp.height);
  await page.keyboard.press('Escape');
  await expect(menu).toHaveCount(0);
});
