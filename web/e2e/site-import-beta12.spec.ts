import { test, expect } from './util-test';
import { url, expectClean } from './util';
test('хост импортирует страницу и видит отчёт без обещания готового видео', async ({page})=>{
  let received='';
  await page.route('**/api/host/sites/import',route=>{
    received=route.request().postDataJSON().url;
    return route.fulfill({json:{added:2,existing:1,skipped:[{url:'https://example.org/3_film/',reason:'Страница недоступна'}],note:'Доступность видео проверяется при открытии'}});
  });
  await page.goto(url('/admin?tab=settings'));
  await page.getByRole('button',{name:'Gidonline',exact:true}).click();
  await expect(page.getByLabel('Страница каталога или фильма')).toHaveValue('https://ag.gidonline.fun/home/');
  await page.getByRole('button',{name:'Подключить фильмы',exact:true}).click();
  await expect(page.getByText('Добавлено: 2 · Уже подключено: 1 · Пропущено: 1')).toBeVisible();
  expect(received).toBe('https://ag.gidonline.fun/home/');
  await page.getByText('Причины пропусков',{exact:true}).click();
  await expect(page.locator('details .reason')).toContainText('Страница недоступна');
  await expect(page.locator('details .reason')).toBeVisible();
  await expectClean(page,'импорт сайта');
});
