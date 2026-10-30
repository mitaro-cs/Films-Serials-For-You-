import { expect, test } from './util-test';
import { url, expectClean } from './util';

test('все части видны, выбор меняет кадр и ведёт к фильму', async ({page}) => {
  const ids=['tt1111111','tt2222222'];
  const cards=ids.map((id,i)=>({id,kind:'movie',title:`Фильм ${i+1}`,original:null,year:2020+i,rating:8,poster:`/logo.svg?poster=${i}`,genres:['Драма'],local:null,mark:null,wished:false}));
  await page.route('**/api/franchise?*',route=>route.fulfill({json:[{name:'История',items:cards}]}));
  await page.route('**/api/discover/title/tt*',route=>{
    const id=new URL(route.request().url()).pathname.split('/').pop();
    const i=ids.indexOf(id ?? '');
    return route.fulfill({json:{card:cards[i],backdrop:`/logo.svg?frame=${i}`,overview:`Описание части ${i+1}`,runtime:120,refreshing:false}});
  });
  await page.goto(url(`/franchise/${ids[0]}`));
  await expect(page.getByRole('heading',{name:'История'})).toBeVisible();
  await expect(page.getByRole('link',{name:/Фильм 1/})).toBeVisible();
  await expect(page.locator('.frame img').last()).toHaveAttribute('src',/frame=0/);
  await page.getByRole('link',{name:/Фильм 2/}).focus();
  await expect(page.locator('.frame img').last()).toHaveAttribute('src',/frame=1/);
  await expect(page.getByRole('link',{name:/Фильм 2/})).toHaveAttribute('href',`/ext/${ids[1]}`);
  // Слева - кадр, постер и описание выбранной части.
  await expect(page.getByText('Описание части 2')).toHaveCount(1);
  await expectClean(page,'страница серии фильмов');
});
