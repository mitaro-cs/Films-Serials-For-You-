import { expect, test } from './util-test';
import { env, expectClean, url } from './util';

const card = { id: 'tt0903747', kind: 'series', title: 'Во все тяжкие', original: 'Breaking Bad', year: 2008,
  rating: 9.5, poster: '/logo.svg', genres: ['Драма'], local: null, mark: 'watching', wished: false };

test('главная показывает подборки и планы даже без локальных файлов', async ({ page }, info) => {
  await page.route('**/api/library', (r) => r.fulfill({ json: { items: [], recent: [], counts: { all: 0, want: 0, watching: 0, watched: 0 } } }));
  await page.route('**/api/discover', (r) => r.fulfill({ json: { rows: [{ key: 'popular_series', title: 'Популярные сериалы', items: [card] }] } }));
  await page.route('**/api/me/marks', (r) => r.fulfill({ json: { want: [], watching: [{ ext: card }], watched: [] } }));
  await page.route('**/api/discover/title/*', (r) => r.fulfill({ json: { card, overview: 'История учителя, чья жизнь меняется.', backdrop: null, posterLarge: null } }));
  await page.goto(url('/'));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(card.title);
  await expect(page.getByRole('region', { name: 'Смотрю сейчас' })).toContainText(card.title);
  await expect(page.getByRole('region', { name: 'Популярные сериалы' })).toContainText(card.title);
  await expectClean(page, 'главная без локальной библиотеки');
  await page.screenshot({ path: `test-results/home-empty-local-${info.project.name}.png` });
});

test('при сбое IMDb сохранённые планы остаются большой рекомендацией', async ({ page }) => {
  await page.route('**/api/library', (r) => r.fulfill({ json: { items: [], recent: [], counts: {} } }));
  await page.route('**/api/discover', (r) => r.fulfill({ status: 503, headers: { 'x-fsfy': '1' }, json: {} }));
  await page.route('**/api/me/marks', (r) => r.fulfill({ json: { want: [{ ext: { ...card, mark: 'want' } }], watching: [], watched: [] } }));
  await page.route('**/api/discover/title/*', (r) => r.fulfill({ status: 503, headers: { 'x-fsfy': '1' }, json: {} }));
  await page.goto(url('/'));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(card.title);
  await expect(page.getByRole('region', { name: 'В планах на вечер' })).toContainText(card.title);
  await expectClean(page, 'главная с сохранёнными планами без IMDb');
});

test('при недоступном IMDb главная предлагает поиск и повтор загрузки', async ({ page }) => {
  await page.route('**/api/library', (r) => r.fulfill({ json: { items: [], recent: [], counts: {} } }));
  await page.route('**/api/discover', (r) => r.fulfill({ status: 503, headers: { 'x-fsfy': '1' }, json: { message: 'Подборки недоступны' } }));
  await page.route('**/api/me/marks', (r) => r.fulfill({ json: { want: [], watching: [], watched: [] } }));
  await page.goto(url('/'));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Что посмотрим сегодня?');
  await expect(page.getByRole('heading', { name: 'Подборки временно недоступны' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Обновить подборки' })).toBeVisible();
  await expectClean(page, 'главная при недоступном источнике');
});

test('биография, дата рождения и резервная фильмография человека', async ({ page }, info) => {
  await page.route('**/api/people/imdb?*', (r) => r.fulfill({ json: {
    id: 'nm1466859', name: 'Owain Yeoman', imdbName: 'Owain Yeoman', role: 'actor', photo: null, works: [card], local: [], availability: 'partial',
    info: { nameRu: 'Оуайн Йомен', biography: 'Валлийский актёр. Известен по роли Уэйна Ригсби в сериале «Менталист».', birthDate: '1978-07-02', source: 'https://www.wikidata.org/wiki/Q355049' },
  } }));
  await page.goto(url('/person?id=nm1466859&name=Owain%20Yeoman&role=actor'));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Оуайн Йомен');
  await expect(page.getByText('Дата рождения: 2 июля 1978 г.')).toBeVisible();
  await expect(page.getByText(/Валлийский актёр/)).toBeVisible();
  await expect(page.getByRole('link', { name: 'Викиданные', exact: true })).toHaveAttribute('href', 'https://www.wikidata.org/wiki/Q355049');
  await expect(page.getByText('0 работ')).toHaveCount(0);
  await expectClean(page, 'страница актёра');
  await page.screenshot({ path: `test-results/person-${info.project.name}.png` });
});

test('информативный график профиля: значения и выбор месяца', async ({ page }, info) => {
  await page.goto(url(`/people/${env().people.me}`));
  const chart = page.getByRole('region', { name: 'По месяцам' });
  await chart.scrollIntoViewIfNeeded();
  await expect(chart.getByRole('button')).toHaveCount(12);
  await chart.getByRole('button').last().click();
  await expect(chart.getByRole('button').last()).toHaveAttribute('aria-pressed', 'true');
  await expect(chart.locator('.chart-detail')).toContainText('досмотрено');
  await expectClean(page, 'график просмотров');
  await page.screenshot({ path: `test-results/stats-${info.project.name}.png` });
});
