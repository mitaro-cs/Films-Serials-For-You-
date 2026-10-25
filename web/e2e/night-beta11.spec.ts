import { test, expect } from './util-test';
import { url, expectClean } from './util';

test('ночной режим: диагностика подсветки, восстановление и команда экрана', async ({page}, info) => {
  const actions:{action:string;lights:boolean}[]=[];
  await page.route('**/api/host/night', route => {
    const body=route.request().postDataJSON(); actions.push(body);
    return route.fulfill({json:body.action==='probe'?{ok:true,supported:2}:body.action==='restore'?{ok:true,changed:2}:{ok:true,displayScheduled:true,lights:{ok:true,changed:2}}});
  });
  await page.goto(url('/admin?tab=settings'));
  const region=page.getByRole('region',{name:'Ночной режим хоста'});
  await expect(region).toBeVisible();
  await region.getByRole('button',{name:'Проверить подсветку MSI'}).click();
  await expect(region.getByRole('checkbox')).toBeChecked();
  await region.getByRole('button',{name:'Погасить экран',exact:true}).click();
  await expect(region.getByRole('status')).toContainText('Через 3 секунды');
  expect(actions.at(-1)).toEqual({action:'off',lights:true});
  await region.getByRole('button',{name:'Вернуть подсветку'}).click();
  await expect(region.getByRole('status')).toContainText('Яркость восстановлена');
  await page.evaluate(()=>scrollTo(0,0));
  await expectClean(page,'ночной режим');
  await region.screenshot({path:`test-results/night-${info.project.name}.png`});
});

test('ошибка Mystic Light показана честно, экран доступен отдельно', async ({page}) => {
  await page.route('**/api/host/night', route => route.fulfill({json:{ok:false,error:'Mystic Light не ответил'}}));
  await page.goto(url('/admin?tab=settings'));
  const region=page.getByRole('region',{name:'Ночной режим хоста'});
  await region.getByRole('button',{name:'Проверить подсветку MSI'}).click();
  await expect(region.getByRole('alert')).toHaveText('Mystic Light не ответил');
  await expect(region.getByRole('checkbox')).toHaveCount(0);
  await expect(region.getByRole('button',{name:'Погасить экран',exact:true})).toBeEnabled();
});
