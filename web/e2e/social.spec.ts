import { expect, test } from './util-test';
import { env, stranger, url, expectHome } from './util';

// Люди: отзывы с оценкой, дружба, переписка с фильмом, регистрация без приглашения.

test('отзыв со звёздами появляется сразу и считается в средней', async ({ page }) => {
  // Лишние отзывы с прошлых прогонов не мешают: считаем по своему.
  await page.goto(url(`/title/${env().ids['Паразиты']}`));
  const reviews = page.locator('#reviews');
  await reviews.scrollIntoViewIfNeeded();
  await reviews.getByRole('slider', { name: 'Оценка' }).focus();
  for (let i = 0; i < 8; i++) await page.keyboard.press('ArrowRight');
  const text = `Проверка ${Date.now()}`;
  await reviews.getByPlaceholder('Что думаете?').fill(text);
  await reviews.getByRole('button', { name: 'Опубликовать' }).click();
  const card = reviews.locator('.review', { hasText: text });
  await expect(card).toBeVisible();
  await expect(card).toContainText('4');
  // Убрать за собой: удалить свой отзыв.
  await card.getByRole('button', { name: 'Удалить отзыв' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Удалить', exact: true }).click();
  await expect(card).toHaveCount(0);
});

test('страница фильма: оценки, актёры, отзывы друзей', async ({ page }) => {
  await page.goto(url(`/title/${env().ids['Ла-Ла Ленд']}`));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Ла-Ла Ленд');
  await expect(page.locator('.score .big')).toContainText('4,3');
  await expect(page.locator('#reviews .review')).toHaveCount(3);
  await expect(page.locator('#reviews')).toContainText('Анна Смирнова');
  await expect(page.getByRole('region', { name: 'Актёры и съёмочная группа' })).toContainText('Дэмьен Шазелл');
});

test('люди: заявка в друзья, принять, написать', async ({ page }, info) => {
  // Свой человек на каждый прогон: дружба меняется, демо-данные - нет.
  const name = `Гость ${info.project.name} ${Date.now() % 100000}`;
  const guest = await stranger(name);
  await guest.c.post(`/api/friends/${env().people.me}`);
  await page.goto(url('/people'));
  const row = page.locator('li', { hasText: name });
  await expect(page.getByRole('region', { name: 'Хотят дружить' })).toContainText(name);
  await row.getByRole('button', { name: 'Принять' }).click();
  await expect(row.getByRole('link', { name: 'Написать' })).toBeVisible();
  await row.getByRole('link', { name: 'Написать' }).click();
  await expect(page).toHaveURL(/\/chats\/\d+/);
  const text = `Привет, Борис ${Date.now()}`;
  await page.getByRole('textbox', { name: 'Сообщение' }).fill(text);
  await page.keyboard.press('Enter');
  await expect(page.locator('.msg.mine', { hasText: text })).toBeVisible();
  await expect(page.locator('.msg.mine', { hasText: text })).not.toHaveClass(/pending/);
  // Ответ приходит сам (опрос раз в 3 с).
  await guest.c.post(`/api/chats/${env().people.me}`, { data: { body: 'И тебе привет!' } });
  await expect(page.locator('.msg:not(.mine)', { hasText: 'И тебе привет!' })).toBeVisible({ timeout: 8000 });
  await guest.c.dispose();
});

test('переписка: фильм карточкой и ответы', async ({ page }) => {
  await page.goto(url(`/chats/${env().people.anna}`));
  await expect(page.locator('.msg').first()).toContainText('Дюна');
  await expect(page.locator('.msg .film').first()).toHaveAttribute('href', `/title/${env().ids['Дюна']}`);
  await page.getByRole('button', { name: 'Приложить фильм' }).click();
  // Сразу, без ввода - то, что смотрю и хочу посмотреть.
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByRole('heading', { name: 'Смотрю' })).toBeVisible();
  await expect(dialog.getByRole('button', { name: /Прислать: Начало/ })).toBeVisible();
  await page.getByRole('textbox', { name: 'Название фильма' }).fill('Амели');
  // Своё - первым, дальше - то же название с IMDb.
  await dialog.getByRole('button', { name: /Амели/ }).first().click();
  await expect(page.locator('.msg.mine .film', { hasText: 'Амели' }).last()).toBeVisible();
});

test('регистрация без приглашения', async ({ browser }) => {
  const ctx = await browser.newContext({ storageState: { cookies: [], origins: [] } });
  const page = await ctx.newPage();
  await page.goto(env().publicURL + '/login');
  await page.getByRole('link', { name: 'Создать аккаунт' }).click();
  await expect(page.getByLabel('Код приглашения')).toHaveCount(0);
  const login = `new${Date.now() % 100000}`;
  await page.getByLabel('Как вас зовут').fill('Новенький');
  await page.getByLabel(/Логин/).fill(login);
  await page.getByLabel(/Пароль/).fill('пароль-12345');
  await page.getByRole('button', { name: 'Создать аккаунт' }).click();
  await expectHome(page);
  await ctx.close();
});
