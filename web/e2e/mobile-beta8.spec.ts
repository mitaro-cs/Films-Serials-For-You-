import {test,expect} from './util-test';
import {url,expectClean} from './util';

test('хост сбрасывает пароль без почты и видит результат только после успеха',async({page})=>{
 await page.route('**/api/admin/users',route=>route.fulfill({json:[{id:7654,login:'viewer',name:'Зритель',role:'viewer',canDownload:false,disabled:false,seenAt:null,hasPassword:true}]}));
 let password='';
 await page.route('**/api/admin/users/7654',route=>{password=route.request().postDataJSON().password;return route.fulfill({json:{ok:true}})});
 await page.goto(url('/admin?tab=people'));
 await page.getByRole('button',{name:'Сбросить пароль',exact:true}).click();
 await page.getByRole('button',{name:'Сменить',exact:true}).click();
 await expect(page.locator('section[role=status]')).toContainText('Новый пароль · Зритель');
 expect(password).toMatch(/^[A-Za-z2-9]{18}$/);
 await expect(page.locator('section[role=status]')).toContainText(password);
 await page.evaluate(()=>scrollTo(0,0));
 await expectClean(page,'пароль сброшен хостом');
 await page.getByRole('button',{name:'Скрыть',exact:true}).click();
 await expect(page.getByText(password,{exact:true})).toHaveCount(0);
});

test('восстановление без SMTP объясняет сброс владельцем',async({page,context})=>{
 await context.clearCookies();
 await page.route('**/api/site',async route=>{const r=await route.fetch();await route.fulfill({json:{...await r.json(),emailRecovery:false}})});
 await page.goto(url('/forgot-password'));
 await expect(page.getByText('Пароль может сбросить владелец вашей Nova. Почта не нужна.')).toBeVisible();
 await expect(page.getByRole('link',{name:'Вернуться ко входу'})).toBeVisible();
 await expect(page.getByLabel('Электронная почта')).toHaveCount(0);
 await expectClean(page,'восстановление без почты');
});

test('богатая карточка фильма без пересечения постера и описания',async({page},info)=>{
 const id='tt0468569';
 const card={id,kind:'movie',title:'Тёмный рыцарь',original:'The Dark Knight',year:2008,rating:9.1,poster:'/icon-512.png',genres:['Криминал','Триллер'],local:null,mark:null,wished:false};
 const detail={card,overview:'Банда преступников в масках клоунов совершает ограбление банка в Готэм-Сити. '.repeat(12),runtime:152,votes:100000,backdrop:'/icon-512.png',posterLarge:'/icon-512.png',tagline:'Классно…',age:'16+',releaseDate:null,countries:['Соединённые Штаты Америки','Великобритания'],languages:['Английский'],aka:[],people:[],gallery:[],similar:[],localCard:null,lists:[],seasons:[],friends:[],links:[]};
 await page.route(`**/api/discover/title/${id}**`,route=>{const p=new URL(route.request().url()).pathname;return route.fulfill({json:p.endsWith('/trailer')?{linkId:null}:p.endsWith('/reviews')?{average:null,votes:0,total:0,items:[],canWrite:false}:detail})});
 await page.route('**/api/franchise?*',route=>route.fulfill({json:[]}));
 await page.goto(url(`/ext/${id}`));
 await expect(page.getByRole('heading',{name:'Тёмный рыцарь',exact:true})).toBeVisible();
 await expectClean(page,'полная карточка фильма');
 // Телефон - как на макете: постер слева, текст справа; они не пересекаются.
 if((page.viewportSize()?.width??0)<=640){const poster=await page.locator('.rwh .poster').boundingBox(), body=await page.locator('.rwh .text').boundingBox();expect(poster!.x+poster!.width).toBeLessThanOrEqual(body!.x+3);}
 await page.screenshot({path:`test-results/movie-beta8-${info.project.name}.png`,fullPage:true});
});

for (const kind of ['rutube','direct']) test(`Safari ${kind}: качества доступны и ручной выбор возвращается в Авто`,async({page})=>{
 await page.addInitScript(()=>{
  // This fixture tests native quality URLs, not media decoding: its manifest has
  // no actual segments. Keep unsupported Windows native playback from retrying it.
  document.addEventListener('error',event=>{if(event.target instanceof HTMLMediaElement)event.stopImmediatePropagation()},true);
  const original=HTMLMediaElement.prototype.canPlayType;
  HTMLMediaElement.prototype.canPlayType=function(type){return type.includes('mpegurl')?'probably':original.call(this,type)};
  Object.defineProperty(window,'MediaSource',{value:undefined});
  // The production native branch also checks presence rather than its value.
  delete (window as any).MediaSource;
 });
 await page.route('**/api/links/987650',route=>route.fulfill({json:{id:987650,kind,url:kind === 'direct' ? 'https://video.example/film.m3u8' : 'https://rutube.ru/video/test',label:'Тест качества',progress:null}}));
 await page.route('**/api/links/987650/stream/source',route=>route.fulfill({json:{url:'/api/links/987650/stream/index.m3u8',hls:true}}));
 await page.route('**/api/links/987650/stream/index.m3u8*',route=>route.fulfill({contentType:'application/vnd.apple.mpegurl',body:'#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=1280x720\n720.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=300000,RESOLUTION=640x360\n360.m3u8'}));
 await page.goto(url('/watch/link/987650'));
 const settings=page.getByRole('button',{name:'Настройки плеера',exact:true});
 await settings.click(); await page.getByRole('menuitem',{name:/Качество/}).click();
 await page.getByRole('menuitemradio',{name:'720p',exact:true}).click();
 await expect(page.locator('video')).toHaveAttribute('src',/height=720/);
 await settings.click();await page.getByRole('menuitem',{name:/Качество/}).click();
 await page.getByRole('menuitemradio',{name:/Авто/}).click();
 await expect(page.locator('video')).not.toHaveAttribute('src',/height=/);
});
