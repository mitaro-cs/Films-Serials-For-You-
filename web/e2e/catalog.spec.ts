import { expect, test } from './util-test';
import { env, resetStatuses, url } from './util';

// Главная как у Apple TV: герой-карусель, ряды широких карточек, вкладки Фильмы и Сериалы
// с фильтром по статусу. Демо-данные общие: перед каждым тестом статусы — как после setup.

test.beforeEach(resetStatuses);

test('главная: герой, ряды и сериалы рядом с фильмами', async ({ page }) => {
  await page.goto(url('/'));
  const hero = page.getByRole('region', { name: 'Рекомендуем' });
  await expect(hero.getByRole('heading', { level: 1 })).toBeVisible();
  const dots = hero.getByRole('tab');
  expect(await dots.count()).toBeGreaterThan(1);
  await dots.nth(1).click();
  await expect(dots.nth(1)).toHaveAttribute('aria-selected', 'true');

  // Где остановился — в «Продолжить просмотр», у сериала — следующая серия.
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

  // «Хочу» на странице фильма — и он в фильтре.
  await page.goto(url(`/title/${env().ids['Амели']}`));
  await page.getByRole('button', { name: /Хочу/ }).first().click();
  await page.goto(url('/?tab=movies&status=want'));
  await expect(page.getByRole('region', { name: 'Вся коллекция' }).getByRole('link', { name: /Амели/ })).toBeVisible();

  await page.goto(url('/?tab=movies&status=watched'));
  await expect(page.getByRole('region', { name: 'Вся коллекция' }).getByRole('link', { name: /Ла-Ла Ленд/ })).toContainText('Просмотрено');
});

test('совместный список: из фильма — в список с другом', async ({ page }) => {
  const name = `Пятница ${test.info().project.name} ${Date.now() % 100000}`;
  await page.goto(url(`/title/${env().ids['Дюна']}`));
  await page.getByRole('button', { name: 'В общий список' }).click();
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

test('Esc в плеере — назад в сервис, а не из приложения', async ({ page }) => {
  await page.goto(url(`/title/${env().ids['Начало']}`));
  await page.getByRole('link', { name: /Продолжить|Смотреть/ }).first().click();
  await expect(page).toHaveURL(/\/watch\//);
  await expect(page.getByRole('button', { name: 'Во весь экран' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page).toHaveURL(/\/title\//);
});

test('видео по ссылке: в плеере Nova, Esc — назад', async ({ page }) => {
  const id = env().ids['Амели'];
  await page.goto(url(`/title/${id}`));
  await page.getByRole('region', { name: 'Видео по ссылке' }).getByRole('button', { name: 'Добавить' }).click();
  const d = page.getByRole('dialog', { name: 'Смотреть по ссылке' });
  await d.getByLabel('Ссылка или название').fill(`https://example.com/amelie-${test.info().project.name}.mp4`);
  await d.getByRole('button', { name: 'Смотреть' }).click();
  await expect(page).toHaveURL(/\/watch\/link\/\d+/);
  await expect(page.getByTitle('Назад (Esc)')).toBeVisible();
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
