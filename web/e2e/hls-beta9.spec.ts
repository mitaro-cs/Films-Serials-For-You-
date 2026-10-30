import {test,expect} from './util-test';
import {url} from './util';
import {execFileSync} from 'node:child_process';
import {mkdtempSync,mkdirSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

test('HLS: ручное качество и кадр сохраняются на паузе, загрузка сегментов останавливается',async({page},info)=>{
 test.skip(info.project.name!=='desktop','Проверка декодирования VP9 через MSE в Chromium');
 test.setTimeout(90000);
 const temp=mkdtempSync(join(tmpdir(),'nova-hls-'));
 try {
  for(const [name,size] of [['low','320x172'],['high','640x344']]) {
   const dir=join(temp,name);mkdirSync(dir);
   execFileSync('ffmpeg',['-hide_banner','-loglevel','error','-y','-f','lavfi','-i',`testsrc2=size=${size}:rate=12:duration=40`,'-c:v','libvpx-vp9','-deadline','realtime','-cpu-used','8','-g','24','-an','-f','hls','-hls_time','2','-hls_list_size','0','-hls_segment_type','fmp4','index.m3u8'],{cwd:dir});
  }
  writeFileSync(join(temp,'master.m3u8'),'#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=120000,CODECS="vp09.00.10.08",RESOLUTION=320x172\nlow/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=900000,CODECS="vp09.00.10.08",RESOLUTION=640x344\nhigh/index.m3u8\n');
  let segments=0;
  await page.route('**/hls-beta9/**',route=>{
   const name=new URL(route.request().url()).pathname.split('/hls-beta9/')[1];
   if(name.endsWith('.m4s'))segments++;
   return route.fulfill({contentType:name.endsWith('.m3u8')?'application/vnd.apple.mpegurl':'video/mp4',body:readFileSync(join(temp,name))});
  });
  await page.route('**/api/links/989897',route=>route.fulfill({json:{id:989897,kind:'direct',url:'/hls-beta9/master.m3u8',label:'Проверка HLS',progress:null}}));
  await page.goto(url('/watch/link/989897'));
  await expect.poll(()=>page.locator('video').evaluate((v:HTMLVideoElement)=>v.readyState)).toBeGreaterThanOrEqual(2);
  await expect.poll(()=>page.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime)).toBeGreaterThan(0);
  await page.mouse.move(100,150);
  await page.locator('.controls .cbtn[aria-label="Пауза"]').click();
  const settings=page.getByRole('button',{name:'Настройки плеера',exact:true});
  await settings.click();await page.getByRole('menuitem',{name:/Качество/}).click();
  await page.getByRole('menuitemradio',{name:'360p',exact:true}).click();
  const pausedAt=await page.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime);
  await page.locator('video').evaluate(v=>{(window as any).__hlsReloads=0;v.addEventListener('loadstart',()=>{(window as any).__hlsReloads++})});
  await page.waitForTimeout(1500);const settledSegments=segments;await page.waitForTimeout(2500);
  expect(segments).toBe(settledSegments);
  expect(await page.locator('video').evaluate((v:HTMLVideoElement)=>v.readyState)).toBeGreaterThanOrEqual(2);
  expect(await page.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
  expect(Math.abs(await page.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime)-pausedAt)).toBeLessThan(.25);
  // Панель прячется через 4 с и на паузе - двигаем мышью, как зритель.
  await page.mouse.move(120,160);
  await settings.click();await page.getByRole('menuitem',{name:/Качество/}).click();
  await expect(page.getByRole('menuitemradio',{name:'360p',exact:true})).toHaveAttribute('aria-checked','true');
  await page.keyboard.press('Escape');
  await page.locator('.controls .cbtn[aria-label="Смотреть"]').click();
  await expect.poll(()=>page.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime)).toBeGreaterThan(pausedAt+1);
  expect(await page.evaluate(()=>(window as any).__hlsReloads)).toBe(0);
 } finally {rmSync(temp,{recursive:true,force:true});}
});
