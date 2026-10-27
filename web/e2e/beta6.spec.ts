import { expect, test } from './util-test';
import { expectClean, url } from './util';

const cards = [
  {id:'tt0133093',title:'Матрица',original:'The Matrix',kind:'movie',year:1999,rating:8.7,genres:['Фантастика'],poster:null,local:null,mark:null,wished:false},
  {id:'tt0234215',title:'Матрица: Перезагрузка',original:'The Matrix Reloaded',kind:'movie',year:2003,rating:7.2,genres:['Боевик'],poster:null,local:null,mark:null,wished:false},
];

test('русский поиск, фильтры и кэш повторного запроса', async ({page}) => {
  let requests = 0;
  await page.route('**/api/discover/search?*', r => { requests++; return r.fulfill({json:cards}); });
  await page.goto(url('/search?q=матрица'));
  const results = page.getByRole('region',{name:'Все фильмы и сериалы'});
  await expect(results.getByRole('link')).toHaveCount(2);
  await page.getByLabel('Рейтинг',{exact:true}).selectOption('8');
  await expect(results.getByRole('link')).toHaveCount(1);
  await expect(results).toContainText('Матрица');
  await page.getByLabel('Рейтинг',{exact:true}).selectOption('');
  await page.getByLabel('Год',{exact:true}).fill('2003');
  await expect(results).toContainText('Перезагрузка');
  await expect(results.getByRole('link')).toHaveCount(1);
  await page.getByLabel('Год',{exact:true}).fill('');
  await page.getByRole('searchbox',{name:'Поиск',exact:true}).fill('');
  await page.getByRole('searchbox',{name:'Поиск',exact:true}).fill('матрица');
  await expect(results.getByRole('link')).toHaveCount(2);
  expect(requests).toBe(1);
  await expectClean(page,'поиск с фильтрами');
});

test('внешнее видео продолжается с точного места, нижняя панель чёрная', async ({page}, info) => {
  await page.route('**/api/continue',r=>r.fulfill({json:[{key:'x:tt0133093',href:'/watch/link/900',position:1224,duration:7200,label:'Фильм',at:1,art:null,ext:{...cards[0],mark:'watching'}}]}));
  await page.goto(url('/'));
  const row = page.getByRole('region',{name:'Продолжить просмотр'});
  await expect(row.getByRole('link')).toHaveAttribute('href','/watch/link/900');
  await expect(row).toContainText('20:24');
  await expect(row.getByRole('meter')).toHaveAttribute('aria-valuenow','17');
  if (info.project.name !== 'desktop') await expect(page.locator('nav.tabs')).toHaveCSS('background-color','rgb(0, 0, 0)');
  await expectClean(page,'продолжение внешнего видео');
});

test('баннер листается в покое и сохраняет ручной выбор', async ({page}) => {
  await page.addInitScript(()=>sessionStorage.removeItem('nova-showcase'));
  await page.route('**/api/library',r=>r.fulfill({json:{items:[],recent:[],counts:{}}}));
  await page.route('**/api/discover',r=>r.fulfill({json:{rows:[{key:'popular_movies',title:'Популярные фильмы',items:cards}]}}));
  await page.route('**/api/me/marks',r=>r.fulfill({json:{want:[],watching:[],watched:[]}}));
  await page.route('**/api/continue',r=>r.fulfill({json:[]}));
  await page.route('**/api/theme',r=>r.fulfill({status:503,headers:{'x-fsfy':'1'},json:{}}));
  await page.route('**/api/discover/title/*',r=>r.fulfill({json:{card:cards.find(c=>r.request().url().endsWith(c.id)),overview:null,backdrop:null,posterLarge:null,refreshing:false}}));
  await page.goto(url('/'));
  const hero=page.getByRole('region',{name:'Рекомендуем'});
  await expect(hero.getByRole('heading',{level:1})).toHaveText('Матрица');
  await page.mouse.move(0,0);
  await expect(hero.getByRole('heading',{level:1})).toHaveText('Матрица: Перезагрузка',{timeout:9000});
  // Точки под кадром: выбрал - карусель стоит.
  await hero.getByRole('tab',{name:'Матрица',exact:true}).click();
  await expect(hero.getByRole('heading',{level:1})).toHaveText('Матрица');
  await page.mouse.move(0,0);
  await page.waitForTimeout(6800);
  await expect(hero.getByRole('heading',{level:1})).toHaveText('Матрица');
});
