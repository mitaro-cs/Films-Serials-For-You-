import { expect, test } from './util-test';
import { expectClean, url } from './util';

test('английские названия из старого кеша не появляются в русском режиме',async({page},info)=>{
 const id='tt3543502';
 const card={id,kind:'series',title:'Гангстерленд',original:'MobLand',year:2025,rating:8.3,poster:null,genres:['Криминал'],local:null,mark:null,wished:false};
 const detail={card,overview:'История противостояния двух семей.',runtime:50,votes:1000,backdrop:null,posterLarge:null,tagline:'Power comes at a price',age:'18+',releaseDate:null,countries:['Великобритания'],languages:['Английский'],aka:[],people:[],gallery:[],similar:[],localCard:null,lists:[],seasons:[1],friends:[],links:[]};
 await page.route(`**/api/discover/title/${id}**`,route=>{
  const path=new URL(route.request().url()).pathname;
  const json=path.endsWith('/episodes')?{items:['STICK OR TWIST','JIGSAW PUZZLE','План Б'].map((name,i)=>({season:1,episode:i+1,name,overview:'An English synopsis',still:null,airDate:null,runtime:50,rating:8,seen:false})),refreshing:false}:path.endsWith('/reviews')?{average:null,votes:0,total:0,items:[],canWrite:false}:path.endsWith('/ratings')?{seasons:[]}:path.endsWith('/trailer')?{linkId:null}:detail;
  return route.fulfill({json});
 });
 await page.route('**/api/franchise?*',route=>route.fulfill({json:[]}));
 await page.route('**/api/links/search?*',route=>route.fulfill({json:[]}));
 await page.goto(url(`/ext/${id}`));
 const episodes=page.getByRole('region',{name:'Серии',exact:true});
 await expect(episodes.getByRole('button',{name:'1 серия: Серия',exact:true})).toBeVisible();
 await expect(episodes.getByText('3. План Б',{exact:true})).toBeVisible();
 await expect(episodes).not.toContainText('STICK OR TWIST');
 await expect(episodes).not.toContainText('JIGSAW PUZZLE');
 await expect(episodes).not.toContainText('An English synopsis');
 await expect(page.getByText('Power comes at a price',{exact:true})).toHaveCount(0);
 await page.screenshot({path:`test-results/russian-episodes-${info.project.name}.png`});
});

test('серии: отдельные источники, превью и длинное название в одну строку', async ({ page }, info) => {
  const id = 'tt1234567';
  const card = { id, kind: 'series', title: 'Сверхъестественное', original: 'Supernatural', year: 2005,
    rating: 8.4, poster: null, genres: ['Драма'], local: null, mark: null, wished: false };
  const detail = { card, overview: 'История братьев и их приключений.', runtime: 44, votes: 1000,
    backdrop: null, posterLarge: null, tagline: null, age: '16+', releaseDate: null, countries: ['США'],
    languages: ['Русский'], aka: [], people: [], gallery: [], similar: [], localCard: null, lists: [], seasons: [1], friends: [],
    links: [1, 2].map((episode) => ({ id: episode, kind: 'direct', url: `https://example.com/${episode}.mp4`,
      embed: null, label: `Источник серии ${episode}`, thumb: null, duration: 2600, titleId: null, extId: id,
      addedBy: null, author: '', addedAt: 0, season: 1, episode, progress: episode === 2 ? { position: 123, duration: 2600 } : null })) };
  await page.route(`**/api/discover/title/${id}**`, (route) => {
    const path = new URL(route.request().url()).pathname;
    const json = path.endsWith('/episodes') ? { items: [1, 2].map((episode) => ({
      season: 1, episode, name: `История ${episode}`, overview: 'Описание серии', still: `/logo.svg?ep=${episode}`,
      airDate: null, runtime: 44, rating: 8, seen: episode === 1,
    })) } : path.endsWith('/reviews') ? { average: null, votes: 0, total: 0, items: [], canWrite: false }
      : path.endsWith('/ratings') ? { seasons: [] } : detail;
    return route.fulfill({ json });
  });
  await page.route('**/api/franchise?*', (route) => route.fulfill({ json: [] }));
  await page.route('**/api/links/search?*', (route) => route.fulfill({ json: [] }));
  await page.goto(url(`/ext/${id}`));
  const heading = page.getByRole('heading', { name: 'Сверхъестественное', exact: true });
  await expect(heading).toBeVisible();
  await expect(page.getByText('Просмотрено', {exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Посмотрел 1 серию',exact:true})).toHaveAttribute('aria-pressed','true');
  expect(await heading.evaluate((el) => getComputedStyle(el).whiteSpace)).toBe('nowrap');
  expect(await heading.evaluate((el) => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
  await expect(page.getByRole('region', { name: 'Серии', exact: true }).locator('.still img')).toHaveCount(2);
  for (const card of await page.getByRole('region', { name: 'Серии', exact: true }).locator('.card').all()) {
    const still = await card.locator('.still').boundingBox();
    const body = await card.locator('.body').boundingBox();
    expect(still).not.toBeNull();
    expect(body).not.toBeNull();
    // Телефон (как в макете): кадр серии над названием; шире - кадр слева от текста.
    if ((page.viewportSize()?.width ?? 0) <= 640) expect(still!.y + still!.height).toBeLessThanOrEqual(body!.y + 1);
    else expect(still!.x + still!.width).toBeLessThanOrEqual(body!.x + 1);
  }
  await page.getByRole('button', { name: '2 серия: История 2', exact: true }).click();
  const modal = page.getByRole('dialog', { name: '1 сезон · 2 серия' });
  await expect(modal.getByRole('link', { name: 'Продолжить · Источник серии 2' })).toHaveAttribute('href', '/watch/link/2');
  await expect(modal.getByText('Источник серии 1')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(modal).toBeHidden();
  await expect(page.getByRole('heading', { name: 'Смотреть онлайн', exact: true })).toHaveCount(0);
  await expectClean(page, 'сериал из Обзора');
  await page.screenshot({ path: `test-results/series-${info.project.name}.png` });
});
