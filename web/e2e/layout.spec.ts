import { test } from './util-test';
import { env, expectClean, url } from './util';

// Каждая страница на телефоне, планшете и компьютере: без наложений и горизонтальной прокрутки.

const PAGES = [
  ['/', 'главная'],
  ['/?tab=movies', 'фильмы'],
  ['/?tab=series', 'сериалы'],
  ['/?tab=movies&status=want', 'хочу посмотреть'],
  ['/?tab=movies&status=watching', 'смотрю'],
  ['/?tab=movies&status=watched', 'просмотрено'],
  ['/search?q=на', 'поиск'],
  ['/profile', 'аккаунт'],
  ['/lists', 'смотрим вместе'],
  ['/admin', 'управление'],
  ['/admin?tab=folders', 'папки'],
  ['/admin?tab=catalog', 'каталог'],
  ['/admin?tab=people', 'люди'],
  ['/admin?tab=access', 'доступ'],
  ['/admin?tab=settings', 'настройки'],
  ['/admin?tab=version', 'версия'],
] as const;

for (const [path, what] of PAGES) {
  test(`вёрстка: ${what}`, async ({ page }) => {
    await page.goto(url(path));
    await expectClean(page, what);
  });
}

test('вёрстка: страница фильма и сериала', async ({ page }) => {
  for (const t of ['Ла-Ла Ленд', 'Очень длинное название фильма, чтобы проверить переносы строк на узком телефоне', 'Тьма']) {
    await page.goto(url(`/title/${env().ids[t]}`));
    await expectClean(page, t);
  }
});

test('вёрстка: люди, страница человека, чаты', async ({ page }) => {
  const { people } = env();
  for (const [path, what] of [
    ['/people', 'люди'],
    [`/people/${people.anna}`, 'страница Анны'],
    [`/people/${people.me}`, 'своя страница'],
    ['/chats', 'чаты'],
    [`/chats/${people.anna}`, 'переписка'],
  ]) {
    await page.goto(url(path));
    await expectClean(page, what);
  }
});

test('вёрстка: вход (без сессии)', async ({ browser }) => {
  const ctx = await browser.newContext({ storageState: { cookies: [], origins: [] } });
  const page = await ctx.newPage();
  await page.goto(env().publicURL + '/login');
  await expectClean(page, 'вход');
  await ctx.close();
});
