import { test } from './util-test';
import { env, expectClean, url } from './util';

// Каждая страница на телефоне, планшете и компьютере: без наложений и горизонтальной прокрутки.

const PAGES = [
  ['/', 'главная — фильмы'],
  ['/?tab=series', 'сериалы'],
  ['/?tab=new', 'новое'],
  ['/?status=want', 'хочу посмотреть'],
  ['/?status=watching', 'смотрю'],
  ['/?status=watched', 'просмотрено'],
  ['/search?q=на', 'поиск'],
  ['/profile', 'аккаунт'],
  ['/profiles', 'кто смотрит'],
  ['/admin', 'управление'],
  ['/admin?tab=folders', 'папки'],
  ['/admin?tab=catalog', 'каталог'],
  ['/admin?tab=people', 'люди'],
  ['/admin?tab=access', 'доступ'],
  ['/admin?tab=settings', 'настройки'],
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

test('вёрстка: вход (без сессии)', async ({ browser }) => {
  const ctx = await browser.newContext({ storageState: { cookies: [], origins: [] } });
  const page = await ctx.newPage();
  await page.goto(env().publicURL + '/login');
  await expectClean(page, 'вход');
  await ctx.close();
});
