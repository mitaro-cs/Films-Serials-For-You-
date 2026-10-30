import { expect, test } from './util-test';
import { url } from './util';

test('знакомство сохраняет выбор и показывает рекомендации', async ({page}) => {
  await page.addInitScript(() => sessionStorage.setItem('nova-onboarding:1','1'));
  const film={id:'tt7654321',kind:'movie',title:'Тестовый фильм',original:null,year:2024,rating:8,poster:null,genres:['Фантастика'],local:null,mark:null,wished:false};
  await page.route('**/api/discover',route=>route.fulfill({json:{rows:[{key:'popular_movies',title:'Популярное',items:[film,{...film,id:'tt7654322',title:'Другая история'}]}]}}));
  await page.route('**/api/me/favorites',route=>route.fulfill({json:{favorites:['x:tt7654321']}}));
  await page.goto(url('/'));
  const guide=page.getByRole('dialog',{name:'Знакомство с Nova'});
  await expect(guide).toBeVisible();
  await guide.getByRole('button',{name:'Продолжить'}).click();
  await guide.getByRole('button',{name:'Фантастика'}).click();
  await guide.getByRole('button',{name:'Продолжить'}).click();
  await guide.getByRole('button',{name:'Тестовый фильм'}).click();
  await guide.getByRole('button',{name:'Начать смотреть'}).click();
  await expect(guide).toHaveCount(0);
  await expect(page.getByRole('heading',{name:'Вам может понравиться'})).toBeVisible();
  await expect.poll(()=>page.evaluate(()=>localStorage.getItem('nova-genres:1'))).toContain('Фантастика');
});
