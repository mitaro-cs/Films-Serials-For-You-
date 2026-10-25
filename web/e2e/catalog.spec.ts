import { expect, test } from './util-test';
import { act, env, narrow, resetStatuses, url } from './util';

// Главная как у Apple TV: герой-карусель, ряды широких карточек, вкладки Фильмы и Сериалы
// с фильтром по статусу. Демо-данные общие: перед каждым тестом статусы - как после setup.

test.beforeEach(resetStatuses);

test('главная: герой, ряды и сериалы рядом с фильмами', async ({ page }) => {
  await page.goto(url('/'));
  const hero = page.getByRole('region', { name: 'Рекомендуем' });
  await expect(hero.getByRole('heading', { level: 1 })).toBeVisible();
  const before = await hero.getByRole('heading', { level: 1 }).textContent();
  await hero.getByRole('button', { name: 'Следующая рекомендация' }).click();
  await expect(hero.getByRole('heading', { level: 1 })).not.toHaveText(before!);

  // Где остановился - в «Продолжить просмотр», у сериала - следующая серия.
  const go = page.getByRole('region', { name: 'Продолжить просмотр' });
  await expect(go.getByRole('link', { name: /Начало/ })).toContainText('осталось');
  await expect(go.getByRole('link', { name: /Тьма/ })).toContainText('1 сезон, 2 серия');
  // Загруженное хостом видно всем: и фильмы, и сериалы.
  await expect(page.getByRole('region', { name: 'Сериалы' }).getByRole('link', { name: /Тьма/ })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Фильмы' }).getByRole('link', { name: /Дюна/ })).toBeVisible();
});

test('Фильмы: фильтр по статусу, отметки со страницы фильма', async ({ page }, info) => {
  await page.goto(url('/'));
  const nav = info.project.name === 'desktop' ? page.locator('nav.pill') : page.getByRole('navigation', { name: 'Разделы каталога' });
  await nav.getByRole('link', { name: 'Фильмы' }).click();
  await expect(page).toHaveURL(/tab=movies/);
  const chip = (name: string) => page.getByRole('tab', { name: new RegExp(`^${name}`) });
  await expect(chip('Хочу посмотреть')).toContainText('2');
  await chip('Хочу посмотреть').click();
  await expect(page).toHaveURL(/status=want/);
  const all = page.getByRole('region', { name: 'Вся коллекция' });
  await expect(all.getByRole('link', { name: /Дюна/ })).toBeVisible();
  await expect(all.getByRole('link', { name: /Амели/ })).toHaveCount(0);

  // «Хочу» на странице фильма - и он в фильтре.
  await page.goto(url(`/title/${env().ids['Амели']}`));
  await page.getByRole('button', { name: /Хочу/ }).first().click();
  await page.goto(url('/?tab=movies&status=want'));
  await expect(page.getByRole('region', { name: 'Вся коллекция' }).getByRole('link', { name: /Амели/ })).toBeVisible();

  await page.goto(url('/?tab=movies&status=watched'));
  await expect(page.getByRole('region', { name: 'Вся коллекция' }).getByRole('link', { name: /Ла-Ла Ленд/ })).toContainText('Просмотрено');
});

test('быстрые отметки на карточке и «Больше не смотрю» с отменой', async ({ page }) => {
  await page.goto(url('/?tab=movies'));
  const all = page.getByRole('region', { name: 'Вся коллекция' });
  await all.getByRole('button', { name: 'Отметки: Амели' }).click();
  await all.getByRole('button', { name: 'Хочу: Амели' }).click();
  await expect(all.getByRole('button', { name: 'Хочу: Амели' })).toHaveAttribute('aria-pressed', 'true');
  await page.reload();
  await expect(page.getByRole('tab', { name: /^Хочу посмотреть/ })).toContainText('3');

  // Начатое - из «Продолжить» крестиком; «Вернуть» - обратно.
  await page.goto(url('/'));
  const go = page.getByRole('region', { name: 'Продолжить просмотр' });
  await go.getByRole('button', { name: 'Больше не смотрю: Начало' }).click();
  await expect(go.getByRole('link', { name: /Начало/ })).toHaveCount(0);
  await page.getByRole('button', { name: 'Вернуть' }).click();
  await expect(go.getByRole('link', { name: /Начало/ })).toBeVisible();
});

test('сериал: лента серий как у HBO, текущая серия выделена', async ({ page }) => {
  await page.goto(url(`/title/${env().ids['Тьма']}`));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Тьма');
  const eps = page.getByRole('region', { name: 'Серии', exact: true });
  await expect(eps.getByRole('link')).toHaveCount(2);
  // Первая досмотрена - текущая вторая.
  await expect(eps.locator('.cur')).toContainText('2');
  // Телефон (как в макете): на кнопке - сама серия («Сезон 1, серия 2»), на ПК - «Продолжить».
  await expect(page.getByRole('link', { name: /Продолжить|Сезон \d+, серия \d+/ }).first()).toBeVisible();
});

test('смотреть вместе: комната, чат, назад к фильму', async ({ page }) => {
  await page.goto(url(`/title/${env().ids['Начало']}`));
  await act(page, 'Смотреть вместе');
  await expect(page).toHaveURL(/\/room\/[a-z0-9]+/);
  const side = page.getByRole('complementary', { name: 'Смотрим вместе' });
  await page.getByRole('button',{name:'Показать чат',exact:true}).click();
  await expect(side).toBeVisible();
  const text = `Поехали ${Date.now() % 10000}`;
  await side.getByRole('textbox', { name: 'Сообщение в комнату' }).fill(text);
  await side.getByRole('button', { name: 'Отправить' }).click();
  await expect(side.locator('.msg', { hasText: text })).toBeVisible();
  await page.getByRole('button', { name: 'Назад' }).first().click();
  await expect(page).toHaveURL(/\/title\//);
});

test('совместный список: из фильма - в список с другом', async ({ page }) => {
  const name = `Пятница ${test.info().project.name} ${Date.now() % 100000}`;
  await page.goto(url(`/title/${env().ids['Дюна']}`));
  await act(page, 'В общий список');
  await page.getByRole('button', { name: 'Новый список' }).click();
  const form = page.getByRole('dialog', { name: 'Новый совместный список' });
  await form.getByLabel('Название').fill(name);
  await form.getByRole('checkbox').first().check();
  await form.getByRole('button', { name: 'Создать' }).click();
  await expect(page.getByRole('menuitemcheckbox', { name: name })).toHaveAttribute('aria-checked', 'true');

  await page.goto(url('/lists'));
  await page.getByRole('link', { name: new RegExp(name) }).click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(name);
  await page.getByRole('button', { name: 'Посмотрели вместе' }).click();
  await expect(page.getByRole('button', { name: 'Не смотрели' })).toBeVisible();

  // Группы: создать и переложить фильм.
  await page.getByRole('button', { name: 'Новая группа' }).click();
  const g = page.getByRole('dialog', { name: 'Новая группа' });
  await g.getByLabel('Название группы').fill('На выходные');
  await g.getByRole('button', { name: 'Создать' }).click();
  await page.getByRole('combobox', { name: 'Группа' }).selectOption({ label: 'На выходные' });
  await expect(page.getByRole('region', { name: 'На выходные' }).getByRole('link', { name: /Дюна/ })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('region', { name: 'На выходные' }).getByRole('link', { name: /Дюна/ })).toBeVisible();

  await page.getByRole('button', { name: 'Удалить', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Удалить', exact: true }).click();
  await expect(page).toHaveURL(/\/lists$/);
});

test('один аккаунт: без «Кто смотрит?», аватар меняется в аккаунте', async ({ page }) => {
  await page.goto(url('/profile'));
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  const pick = page.getByRole('radiogroup', { name: 'Аватар' }).getByRole('radio');
  await pick.nth(7).click();
  await expect(pick.nth(7)).toHaveAttribute('aria-checked', 'true');
  await pick.nth(0).click();
  await expect(pick.nth(0)).toHaveAttribute('aria-checked', 'true');
});

test('Esc в плеере - назад в сервис, а не из приложения', async ({ page }) => {
  await page.goto(url(`/title/${env().ids['Начало']}`));
  await page.getByRole('link', { name: /Продолжить|Смотреть/ }).first().click();
  await expect(page).toHaveURL(/\/watch\//);
  await expect(page.getByRole('button', { name: 'Во весь экран' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page).toHaveURL(/\/title\//);
});

test('видео по ссылке: в плеере Nova, Esc - назад', async ({ page }) => {
  const id = env().ids['Амели'];
  await page.goto(url(`/title/${id}`));
  await page.getByRole('region', { name: 'Видео по ссылке' }).getByRole('button', { name: 'Добавить' }).click();
  const d = page.getByRole('dialog', { name: 'Смотреть по ссылке' });
  await d.getByLabel('Ссылка или название').fill(`https://example.com/amelie-${test.info().project.name}.mp4`);
  await d.getByRole('button', { name: 'Смотреть', exact: true }).click();
  await expect(page).toHaveURL(/\/watch\/link\/\d+/);
  await expect(page.getByRole('button', { name: 'Назад' }).first()).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page).toHaveURL(new RegExp(`/title/${id}`));
  await expect(page.getByRole('region', { name: 'Видео по ссылке' }).getByRole('link').first()).toBeVisible();
});

test('обзор и профиль: поиск IMDb открывается, оформление сохраняется', async ({ page }) => {
  await page.goto(url('/discover'));
  await expect(page.getByRole('searchbox', { name: 'Поиск по IMDb' })).toBeVisible();
  await page.getByRole('button', { name: 'Смотреть по ссылке' }).click();
  await expect(page.getByRole('dialog', { name: 'Смотреть по ссылке' })).toBeVisible();
  await page.keyboard.press('Escape');

  await page.goto(url('/profile'));
  const colors = page.getByRole('radiogroup', { name: 'Цвет профиля' }).getByRole('radio');
  await colors.nth(3).click();
  await expect(colors.nth(3)).toHaveAttribute('aria-checked', 'true');
  await page.getByLabel(/О себе/).fill('Люблю фантастику');
  await page.getByRole('button', { name: 'Сохранить', exact: true }).first().click();
  await expect(page.getByText('Сохранено')).toBeVisible();
  await page.getByRole('link', { name: 'Моя страница' }).click();
  await expect(page.getByText('Люблю фантастику')).toBeVisible();
  await expect(page.getByRole('region', { name: 'Статистика' })).toBeVisible();
  await page.goto(url('/profile'));
  await page.getByRole('radiogroup', { name: 'Цвет профиля' }).getByRole('radio', { name: 'Без цвета' }).click();
});

test('оценку можно убрать, любимый фильм - на своей странице', async ({ page }) => {
  await page.goto(url(`/title/${env().ids['Амели']}`));
  // Телефон: оценка и «Любимый» - в окне «…».
  if (narrow(page)) await page.getByRole('button', { name: 'Ещё действия' }).click();
  const stars = page.getByRole('slider', { name: 'Ваша оценка' }).first();
  await stars.focus();
  await page.keyboard.press('ArrowRight');
  await expect(stars).toHaveAttribute('aria-valuenow', '0.5');
  await page.getByRole('button', { name: 'Убрать оценку' }).click();
  await expect(stars).toHaveAttribute('aria-valuenow', '0');

  const fav = page.getByRole('button', { name: 'Любимый' }).first();
  if ((await fav.getAttribute('aria-pressed')) !== 'true') await fav.click();
  await expect(fav).toHaveAttribute('aria-pressed', 'true');
  await page.goto(url(`/people/${env().people.me}`));
  await expect(page.getByRole('region', { name: 'Любимые фильмы' }).getByRole('link', { name: /Амели/ })).toBeVisible();
});

test('смотреть вместе: пауза и запуск - с любой вкладки', async ({ page, browser }) => {
  // Демо-видео (H.264) Chromium из Playwright не играет - проверяем, что команды доходят до комнаты.
  await page.goto(url(`/title/${env().ids['Начало']}`));
  await act(page, 'Смотреть вместе');
  await expect(page).toHaveURL(/\/room\/[a-z0-9]+/);
  const room = new URL(page.url()).pathname.split('/').pop();
  const playing = async () => (await (await page.request.get(url(`/api/rooms/${room}`))).json()).state.playing as boolean;
  // Вторая вкладка того же человека - как второй зритель.
  const ctx = await browser.newContext({ storageState: 'e2e/.state.json' });
  const other = await ctx.newPage();
  await other.goto(page.url());
  await other.getByRole('button',{name:'Показать чат',exact:true}).click();
  await expect(other.getByRole('complementary', { name: 'Смотрим вместе' })).toBeVisible();
  await page.locator('video').waitFor();
  await page.keyboard.press('k');
  await expect.poll(playing, { timeout: 8000 }).toBe(true);
  // Пауза со второй вкладки - сразу после команды первой (раньше в окне сверки она терялась).
  await other.locator('video').waitFor();
  await other.keyboard.press('k');
  await expect.poll(playing, { timeout: 8000 }).toBe(false);
  await page.keyboard.press('k');
  await expect.poll(playing, { timeout: 8000 }).toBe(true);
  await page.keyboard.press('k');
  await expect.poll(playing, { timeout: 8000 }).toBe(false);
  await ctx.close();
});
