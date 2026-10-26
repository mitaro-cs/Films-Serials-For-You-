import { expect, test } from './util-test';
import { env, url } from './util';
import { execFileSync } from 'node:child_process';
import { canDecodeWebm } from './media-capability';

test('русский трейлер с сайта - только по кнопке и со звуком, ручная пауза сохраняется', async ({ page }, info) => {
  test.skip((page.viewportSize()?.width ?? 0) <= 640, 'на телефоне верх страницы фильма - постер как у Disney+, без трейлера');
  const clip = execFileSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=1280x720:rate=12:duration=10', '-c:v', 'libvpx', '-deadline', 'realtime', '-f', 'webm', 'pipe:1'], { maxBuffer: 8 * 1024 * 1024 });
  test.skip(!await canDecodeWebm(page,clip),'VP8 WebM не декодируется этой тестовой средой');
  // Официальный HD-трейлер IMDb (сеть) в этом тесте не нужен - проверяем русский с сайта.
  await page.route('**/api/discover/title/*/trailer/hd', route => route.fulfill({ status: 404, json: {} }));
  await page.route('**/api/discover/title/*/trailer', route => route.fulfill({ json: { linkId: 987654 } }));
  await page.route('**/api/links/987654/stream/source', route => route.fulfill({ json: { url: '/preview-test.webm', hls: false } }));
  await page.route('**/preview-test.webm', route => route.fulfill({ contentType: 'video/webm', body: clip }));
  await page.route(`**/api/titles/${env().ids['Ла-Ла Ленд']}`, async route => { const response = await route.fetch(); const data = await response.json(); data.imdbId = 'tt3783958'; await route.fulfill({json:data}); });
  await page.goto(url(`/title/${env().ids['Ла-Ла Ленд']}`));
  const banner = page.getByRole('region', { name: 'Баннер и трейлер' });
  await expect(banner).toBeVisible();
  const video = banner.locator('video');
  // Официального нет - сам ничего не играет (чужие перезаливки со вшитыми «16+» и рамками), только кнопка.
  await expect(banner.getByRole('button', { name: 'Включить трейлер' })).toBeVisible();
  await page.waitForTimeout(2500);
  expect(await video.evaluate((v: HTMLVideoElement) => v.paused && !v.getAttribute('src'))).toBe(true);
  await banner.getByRole('button', { name: 'Включить трейлер' }).click();
  await expect.poll(() => video.evaluate((v: HTMLVideoElement) => !v.paused && v.currentTime > 0)).toBe(true);
  await banner.getByRole('button', { name: 'Приостановить трейлер' }).click();
  await expect.poll(() => video.evaluate((v: HTMLVideoElement) => v.paused)).toBe(true);
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
  expect(await video.evaluate((v: HTMLVideoElement) => v.paused)).toBe(true);
  expect(await page.locator('iframe').count()).toBe(0);
  await page.screenshot({path: `test-results/trailer-${info.project.name}.png`});
  await page.evaluate(() => scrollTo(0, document.body.scrollHeight));
  await expect.poll(() => video.evaluate((v: HTMLVideoElement) => v.getAttribute('src'))).toBeNull();
  await page.evaluate(() => scrollTo(0, 0));
  await expect(banner.getByRole('button', {name:'Включить трейлер'})).toBeVisible();
  expect(await video.evaluate((v: HTMLVideoElement) => v.paused)).toBe(true);

});

test('ошибка источника оставляет выбор в Nova и не открывает iframe', async ({ page }) => {
  await page.route('**/api/links/987654', route => route.fulfill({ json: { id: 987654, kind: 'rutube', url: 'https://rutube.ru/video/test', embed: 'https://rutube.ru/play/embed/test', label: 'Тест', progress: null } }));
  await page.route('**/api/links/987654/stream/source', route => route.fulfill({ status: 502, json: { error: { code: 'stream', message: 'Поток недоступен' } } }));
  await page.goto(url('/watch/link/987654'));
  await expect(page.getByRole('button', { name: 'Повторить', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Выбрать источник' })).toBeVisible();
  await expect(page.locator('iframe')).toHaveCount(0);
});

test('F скрывает оболочку, внешние серии переключаются без выхода из полноэкранного режима', async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(window, '__NOVA_DESKTOP__', { value: true }));
  const native:boolean[]=[];
  await page.route('**/api/host/fullscreen',route=>{native.push(route.request().postDataJSON().enabled);return route.fulfill({json:{native:true}});});
  const clip=execFileSync('ffmpeg',['-hide_banner','-loglevel','error','-f','lavfi','-i','testsrc2=size=320x180:rate=12:duration=4','-c:v','libvpx','-deadline','realtime','-f','webm','pipe:1']);
  test.skip(!await canDecodeWebm(page,clip),'VP8 WebM не декодируется этой тестовой средой');
  await page.route('**/episode-test.webm',route=>route.fulfill({contentType:'video/webm',body:clip}));
  await page.route(/\/api\/links\/9900[12](?:\/(?:next|progress))?$/,route=>{
    const path=new URL(route.request().url()).pathname,id=path.includes('99001')?99001:99002;
    if(path.endsWith('/progress'))return route.fulfill({json:{ok:true}});
    if(path.endsWith('/next'))return route.fulfill({json:{next:id===99001?{id:99002,title:'Вторая серия'}:null}});
    return route.fulfill({json:{id,kind:'direct',url:'/episode-test.webm',embed:null,label:id===99001?'Первая серия':'Вторая серия',titleId:null,extId:null,season:1,episode:id===99001?1:2,progress:null}});
  });
  await page.goto(url('/watch/link/99001'));
  const player=page.getByRole('application',{name:'Видеоплеер'});
  await expect(player).toBeVisible();
  await page.waitForFunction(()=>!!document.querySelector('video')?.duration);
  await page.locator('video').evaluate(async(v:HTMLVideoElement)=>{v.muted=true;await v.play();});
  await page.keyboard.press('KeyF');
  await expect.poll(()=>page.evaluate(()=>!!document.fullscreenElement)).toBe(true);
  await expect.poll(()=>native.at(-1)).toBe(true);
  await page.locator('video').evaluate(v=>(window as any).__sameVideo=v);
  await expect(page.getByText('Следующая серия',{exact:true})).toBeVisible();
  await expect(page).toHaveURL(/\/watch\/link\/99002$/, {timeout:12000});
  await expect(player.getByText('Вторая серия',{exact:true})).toBeVisible();
  expect(await page.locator('video').evaluate(v=>v===(window as any).__sameVideo)).toBe(true);
  expect(await page.evaluate(()=>!!document.fullscreenElement)).toBe(true);
  await page.keyboard.press('Escape');
  await expect.poll(()=>native.at(-1)).toBe(false);
});
