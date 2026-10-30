import { expect, test } from './util-test';
import { expectClean, url } from './util';

test('будущая серия показывает год выхода без кнопки просмотра и старого кадра', async ({page}) => {
  const id='tt7654323';
  const card={id,kind:'series',title:'Будущий сезон',original:null,year:2024,rating:8,poster:null,genres:[],local:null,mark:null,wished:false};
  const detail={card,overview:'',runtime:45,votes:0,backdrop:null,posterLarge:null,tagline:null,age:null,releaseDate:null,countries:[],languages:[],aka:[],people:[],gallery:[],similar:[],localCard:null,lists:[],seasons:[5],friends:[],links:[]};
  const year=new Date().getFullYear()+2;
  await page.route(`**/api/discover/title/${id}**`,route=>{
    const path=new URL(route.request().url()).pathname;
    return route.fulfill({json:path.endsWith('/episodes')?{items:[{season:5,episode:1,name:'Серия',overview:null,still:'/logo.svg?old-frame',airDate:`${year}-01-01`,runtime:null,rating:null,seen:false}]}:path.endsWith('/reviews')?{average:null,votes:0,total:0,items:[],canWrite:false}:path.endsWith('/ratings')?{seasons:[]}:detail});
  });
  await page.route('**/api/franchise?*',route=>route.fulfill({json:[]}));
  await page.route('**/api/links/search?*',route=>route.fulfill({json:[]}));
  await page.goto(url(`/ext/${id}`));
  const episodes=page.getByRole('region',{name:'Серии',exact:true});
  await expect(episodes.getByText(`Выйдет в ${year} году`)).toBeVisible();
  await expect(episodes.getByRole('button',{name:/1 серия/})).toHaveCount(0);
  await expect(episodes.getByRole('button',{name:/Посмотрел/})).toHaveCount(0);
  await expect(episodes.locator('img[src*="old-frame"]')).toHaveCount(0);
});

test('фильм: другие источники запрашиваются только после пустого LordFilm и нажатия', async ({page}) => {
  const id='tt7654322';
  const card={id,kind:'movie',title:'Тестовый фильм',original:null,year:2024,rating:8,poster:null,genres:[],local:null,mark:null,wished:false};
  const backdrop='/api/discover/art?url=https%3A%2F%2Fm.media-amazon.com%2Fimages%2Fbanner.jpg';
  const detail={card,overview:'Описание фильма.',runtime:100,votes:100,backdrop,posterLarge:null,tagline:null,age:'16+',releaseDate:null,countries:[],languages:[],aka:[],people:[],gallery:[],similar:[],localCard:null,lists:[],seasons:[],friends:[],links:[]};
  await page.route(`**/api/discover/title/${id}**`, route=>{
    const path=new URL(route.request().url()).pathname;
    return route.fulfill({json:path.endsWith('/reviews')?{average:null,votes:0,total:0,items:[],canWrite:false}:path.endsWith('/trailer')?{linkId:null}:detail});
  });
  await page.route('**/api/discover/art?*',route=>route.fulfill({contentType:'image/svg+xml',body:'<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900"><rect width="1600" height="900" fill="#222"/></svg>'}));
  await page.route('**/api/franchise?*',route=>route.fulfill({json:[]}));
  const providers:string[]=[];
  let finish!:()=>void;
  const pending=new Promise<void>(resolve=>{finish=resolve});
  await page.route('**/api/links/search?*',async route=>{
    const q=new URL(route.request().url()).searchParams;
    if(q.get('mode')==='trailer') return route.fulfill({json:[]});
    providers.push(q.get('provider')??'');
    if(q.get('provider')==='lordfilm') await pending;
    return route.fulfill({json:[]});
  });
  await page.goto(url(`/ext/${id}`));
  if ((page.viewportSize()?.width ?? 0) >= 981) {
    const info = page.locator('.rwh .text');
    await expect(info).toBeVisible();
    expect((await info.boundingBox())?.width).toBeGreaterThan(350);
    await expect(page.locator('.rwh .poster')).toBeVisible();
  }
  await expect.poll(()=>providers).toEqual(['lordfilm']);
  await expect(page.getByRole('button',{name:'Найти на другом источнике',exact:true})).toHaveCount(0);
  await expect(page.locator('img[src*="media-amazon.com"]').first()).toBeAttached();
  finish();
  const fallback=page.getByRole('button',{name:'Найти на другом источнике',exact:true}).filter({visible:true}).first();
  await expect(fallback).toBeVisible();
  expect(providers).toEqual(['lordfilm']);
  await fallback.click();
  await expect.poll(()=>providers.includes('rutube')).toBe(true);
  expect(providers.filter(p=>p==='lordfilm')).toHaveLength(1);
});

test('автопоиск серии не сохраняет источник предыдущего эпизода', async ({page}) => {
  const id='tt7654321';
  const card={id,kind:'series',title:'Тестовый сериал',original:null,year:2024,rating:8,poster:null,genres:[],local:null,mark:null,wished:false};
  const detail={card,overview:'Проверка выбора серии.',runtime:45,votes:100,backdrop:null,posterLarge:null,tagline:null,age:'16+',releaseDate:null,countries:[],languages:[],aka:[],people:[],gallery:[],similar:[],localCard:null,lists:[],seasons:[1],friends:[],links:[]};
  await page.route(`**/api/discover/title/${id}**`, route => {
    const path=new URL(route.request().url()).pathname;
    return route.fulfill({json:path.endsWith('/episodes')?{items:[1,2].map(episode=>({season:1,episode,name:`История ${episode}`,overview:null,still:null,airDate:null,runtime:45,rating:null,seen:false}))}:
      path.endsWith('/reviews')?{average:null,votes:0,total:0,items:[],canWrite:false}:path.endsWith('/ratings')?{seasons:[]}:path.endsWith('/trailer')?{linkId:null}:detail});
  });
  await page.route('**/api/franchise?*',route=>route.fulfill({json:[]}));
  let releaseSecond!: () => void;
  const secondReady=new Promise<void>(resolve=>{releaseSecond=resolve});
  const providers: string[] = [];
  await page.route('**/api/links/search?*', async route=>{
    const q=new URL(route.request().url()).searchParams;
    if(q.has('episode')) providers.push(q.get('provider') ?? '');
    if(q.get('provider')!=='lordfilm' || !q.has('episode')) return route.fulfill({json:[]});
    const episode=Number(q.get('episode'));
    if(episode===2) await secondReady;
    return route.fulfill({json:[{site:'lordfilm',url:`https://sd.lordfilm5.pro/serialy/1.html#nova-season=1&episode=${episode}`,label:`Тестовый сериал S01E0${episode}`,duration:2700,thumb:null,thumbUrl:null,author:'LordFilm'}]});
  });
  let saved: Record<string, unknown> | null = null;
  await page.route('**/api/links', async route=>{
    if(route.request().method()!=='POST') return route.continue();
    saved=route.request().postDataJSON();
    return route.fulfill({status:400,json:{error:'test',message:'Проверка завершена'}});
  });
  await page.goto(url(`/ext/${id}`));
  const play=page.getByRole('button',{name:'Смотреть',exact:true}).filter({visible:true}).first();
  await expect(play).toBeVisible();
  expect(providers).toEqual(['lordfilm']);
  await expect(page.getByRole('button',{name:'Найти на другом источнике',exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'2 серия: История 2',exact:true}).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Смотреть',exact:true}).filter({visible:true})).toHaveCount(0);
  releaseSecond();
  await expect(play).toBeVisible();
  await play.click();
  await expect.poll(()=>saved).toMatchObject({extId:id,season:1,episode:2,url:'https://sd.lordfilm5.pro/serialy/1.html#nova-season=1&episode=2'});
  expect(providers.every(p=>p==='lordfilm')).toBe(true);
});

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
    // Удобный вертикальный список: кадр находится слева от описания и на телефоне.
    expect(still!.x + still!.width).toBeLessThanOrEqual(body!.x + 1);
  }
  await page.getByRole('button', { name: '2 серия: История 2', exact: true }).click();
  await page.getByRole('button',{name:'Найти на другом источнике',exact:true}).filter({visible:true}).first().click();
  const modal = page.getByRole('dialog', { name: '1 сезон · 2 серия' });
  await expect(modal.getByRole('link', { name: 'Продолжить · Источник серии 2' })).toHaveAttribute('href', '/watch/link/2');
  await expect(modal.getByText('Источник серии 1')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(modal).toBeHidden();
  await expect(page.getByRole('heading', { name: 'Смотреть онлайн', exact: true })).toHaveCount(0);
  await expectClean(page, 'сериал из Обзора');
  await page.screenshot({ path: `test-results/series-${info.project.name}.png` });
});

test('переключение сезона показывает только его серии без перезагрузки и скачка', async ({page}) => {
 const id='tt7654324';
 const card={id,kind:'series',title:'Выбор сезона',original:null,year:2024,rating:8,poster:null,genres:[],local:null,mark:null,wished:false};
 const detail={card,overview:'',runtime:45,votes:0,backdrop:null,posterLarge:null,tagline:null,age:null,releaseDate:null,countries:[],languages:[],aka:[],people:[],gallery:[],similar:[],localCard:null,lists:[],seasons:[1,2],friends:[],links:[]};
 await page.route(`**/api/discover/title/${id}**`, route=>{
  const u=new URL(route.request().url()), season=Number(u.searchParams.get('season')||1);
  return route.fulfill({json:u.pathname.endsWith('/episodes')?{items:[{season,episode:1,name:`История сезона ${season}`,overview:null,still:null,airDate:null,runtime:45,rating:null,seen:false}],refreshing:false}:u.pathname.endsWith('/reviews')?{average:null,votes:0,total:0,items:[],canWrite:false}:u.pathname.endsWith('/ratings')?{seasons:[]}:detail});
 });
 await page.route('**/api/franchise?*',route=>route.fulfill({json:[]}));
 await page.route('**/api/links/search?*',route=>route.fulfill({json:[]}));
 await page.goto(url(`/ext/${id}`));
 const region=page.getByRole('region',{name:'Серии',exact:true});
 await expect(region.getByText('1. История сезона 1')).toBeVisible();
 const seasonButton=(n:number) => (page.viewportSize()?.width ?? 0) > 1000 ? page.getByRole('navigation',{name:'Сезоны сбоку'}).getByRole('button',{name:`${n} сезон`}) : region.getByRole('tab',{name:`${n} сезон`});
 const tab=seasonButton(2);
 await tab.scrollIntoViewIfNeeded();
 const before=await page.evaluate(()=>scrollY);
 await page.evaluate(()=>{(window as any).__seasonTest=123});
 await tab.click();
 await expect(region.getByText('1. История сезона 2')).toBeVisible();
 await expect(region.getByText('1. История сезона 1')).toHaveCount(0);
 expect(await page.evaluate(()=>(window as any).__seasonTest)).toBe(123);
 expect(Math.abs((await page.evaluate(()=>scrollY))-before)).toBeLessThan(5);
 await seasonButton(1).click();
 await expect(region.getByText('1. История сезона 1')).toBeVisible();
 await expect(region.getByText('1. История сезона 2')).toHaveCount(0);
});

test('постер, кнопка продолжения и описание не пересекаются', async ({page}) => {
 await page.setViewportSize({width:1280,height:800});
 const id='tt7654325'; const card={id,kind:'movie',title:'Американский психопат',original:'American Psycho',year:2000,rating:7.6,poster:'/logo.svg',genres:['Драма'],local:null,mark:null,wished:false};
 const detail={card,overview:'Длинное описание фильма для проверки отступов.',runtime:102,votes:0,backdrop:null,posterLarge:null,tagline:null,age:'18+',releaseDate:null,countries:[],languages:[],aka:[],people:[],gallery:[],similar:[],localCard:null,lists:[],seasons:[],friends:[],links:[{id:1,kind:'direct',url:'https://sd.lordfilm5.pro/film/1.html',embed:null,label:'Фильм',thumb:null,duration:6000,titleId:null,extId:id,addedBy:null,author:'',addedAt:0,season:null,episode:null,progress:{position:500,duration:6000}}]};
 await page.route(`**/api/discover/title/${id}**`,route=>route.fulfill({json:new URL(route.request().url()).pathname.endsWith('/reviews')?{average:null,votes:0,total:0,items:[],canWrite:false}:detail}));
 await page.route('**/api/franchise?*',route=>route.fulfill({json:[]}));
 await page.route('**/api/links/search?*',route=>route.fulfill({json:[]}));
 await page.goto(url(`/ext/${id}`));
 const hero=page.locator('.rwh'); await expect(hero).toBeVisible();
 await expect(hero.getByText('Продолжить просмотр')).toBeVisible();
 const poster=await hero.locator('.poster').boundingBox(), button=await hero.locator('.go .rw-watch').boundingBox(), info=await hero.locator('.text').boundingBox();
 expect(poster!.x+poster!.width).toBeLessThanOrEqual(info!.x-10);
 expect(button!.x+button!.width).toBeLessThanOrEqual(info!.x-10);
 // Плашка справа - во всю высоту кадра по макету: узкая, но не уже трети постера, и не наезжает на текст.
 const panel=await hero.locator('.plate').boundingBox();
 expect(panel!.width).toBeGreaterThan(poster!.width/3);
 expect(info!.x+info!.width).toBeLessThanOrEqual(panel!.x-10);
 // Кадр - на весь первый экран.
 expect((await hero.boundingBox())!.height).toBeGreaterThanOrEqual(page.viewportSize()!.height-2);
});

test('ячейка рейтинга при наведении не перекрывает соседние', async ({page}) => {
 const id='tt7654326'; const card={id,kind:'series',title:'Проверка рейтингов',original:null,year:2020,rating:8,poster:null,genres:[],local:null,mark:null,wished:false};
 const detail={card,overview:'',runtime:45,votes:0,backdrop:null,posterLarge:null,tagline:null,age:null,releaseDate:null,countries:[],languages:[],aka:[],people:[],gallery:[],similar:[],localCard:null,lists:[],seasons:[1,2],friends:[],links:[]};
 await page.route(`**/api/discover/title/${id}**`,route=>{const path=new URL(route.request().url()).pathname;return route.fulfill({json:path.endsWith('/ratings')?{seasons:[1,2].map(season=>({season,episodes:[{episode:1,rating:8.2,name:'История'}]}))}:path.endsWith('/episodes')?{items:[],refreshing:false}:path.endsWith('/reviews')?{average:null,votes:0,total:0,items:[],canWrite:false}:detail})});
 await page.route('**/api/franchise?*',route=>route.fulfill({json:[]}));await page.route('**/api/links/search?*',route=>route.fulfill({json:[]}));
 await page.goto(url(`/ext/${id}`));const grid=page.locator('.rg');await grid.scrollIntoViewIfNeeded();const cells=grid.locator('tbody .c');await expect(cells).toHaveCount(2);await cells.first().hover();
 const a=await cells.first().boundingBox(),b=await cells.nth(1).boundingBox();expect(a!.x+a!.width).toBeLessThanOrEqual(b!.x);
});
