import { expect, test } from './util-test';
import { url } from './util';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { canDecodeWebm } from './media-capability';

test('сенсорные кнопки перематывают, ставят паузу и скрываются вместе с плеером', async ({page}, info) => {
  test.skip(!['phone','iphone-webkit'].includes(info.project.name),'Сенсорный интерфейс');
  const dir=mkdtempSync(join(tmpdir(),'nova-player-test-')), file=join(dir,'clip.webm');
  let clip:Buffer;
  try { execFileSync('ffmpeg',['-hide_banner','-loglevel','error','-f','lavfi','-i','testsrc2=size=320x180:rate=5:duration=35','-c:v','libvpx','-deadline','realtime',file]); clip=readFileSync(file); }
  finally { rmSync(dir,{recursive:true,force:true}); }
  test.skip(!await canDecodeWebm(page,clip),'Нет декодера VP8');
  await page.route('**/mobile-test.webm',r=>{
    const match=/bytes=(\d+)-(\d*)/.exec(r.request().headers().range??'');
    if(!match)return r.fulfill({contentType:'video/webm',headers:{'accept-ranges':'bytes'},body:clip});
    const start=Number(match[1]),end=match[2]?Math.min(Number(match[2]),clip.length-1):clip.length-1;
    return r.fulfill({status:206,contentType:'video/webm',headers:{'accept-ranges':'bytes','content-range':`bytes ${start}-${end}/${clip.length}`},body:clip.subarray(start,end+1)});
  });
  await page.route('**/api/links/99120',r=>r.fulfill({json:{id:99120,kind:'direct',url:'/mobile-test.webm',label:'Проверка мобильного плеера',progress:null}}));
  await page.route('**/api/links/99120/stream/source',r=>r.fulfill({json:{url:'/mobile-test.webm',hls:false}}));
  await page.route('**/api/links/99120/progress',r=>r.fulfill({json:{ok:true}}));
  await page.goto(url('/watch/link/99120'));
  await page.waitForFunction(()=>!!document.querySelector('video')?.duration);
  const controls=page.locator('.touch-playback');
  await page.locator('video').evaluate(async(v:HTMLVideoElement)=>{v.muted=true;await v.play();});
  await page.getByRole('application',{name:'Видеоплеер'}).dispatchEvent('pointermove');
  await controls.getByRole('button',{name:'Пауза',exact:true}).click();
  await expect.poll(()=>page.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
  await controls.getByRole('button',{name:'Вперёд на 10 секунд'}).click();
  await expect.poll(()=>page.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime)).toBeGreaterThan(9);
  await controls.getByRole('button',{name:'Смотреть',exact:true}).click();
  await expect(controls).not.toHaveClass(/visible/,{timeout:7000});
  await page.setViewportSize({width:844,height:390});
  await page.getByRole('application',{name:'Видеоплеер'}).dispatchEvent('pointermove');
  await expect(controls).toHaveClass(/visible/);
  await page.screenshot({path:`test-results/player-beta12-${info.project.name}.png`});
});
