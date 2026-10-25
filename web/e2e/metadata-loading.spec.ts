import { test, expect } from './util-test';
import { env, url } from './util';

test('открытая карточка получает готовое описание без повторного входа', async ({page}) => {
  const id = env().ids['Амели'];
  const detail = await (await page.request.get(url(`/api/titles/${id}`))).json();
  let requests = 0;
  await page.route(`**/api/titles/${id}`, route => {
    requests++;
    return route.fulfill({json:{...detail, metadataPending: requests === 1,
      card:{...detail.card, title: requests === 1 ? 'Описание загружается' : 'Описание готово'}}});
  });
  await page.goto(url(`/title/${id}`));
  await expect(page.getByRole('heading',{level:1,name:'Описание загружается',exact:true})).toBeVisible();
  await expect(page.getByRole('heading',{level:1,name:'Описание готово',exact:true})).toBeVisible({timeout:10000});
  const completed = requests;
  await page.waitForTimeout(5500);
  expect(requests).toBe(completed);
});
