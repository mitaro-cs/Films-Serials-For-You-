import {test,expect} from './util-test';
import {url,env} from './util';
import {execFileSync} from 'node:child_process';
import {mkdtempSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {canDecodeWebm} from './media-capability';

test('десять зрителей: реальные плееры принимают общую паузу без перезапуска',async({page,browser},info)=>{
 test.skip(info.project.name!=='desktop','Десять отдельных браузерных контекстов проверяем на компьютере');
 test.setTimeout(180000);
 const temp=mkdtempSync(join(tmpdir(),'nova-ten-'));const file=join(temp,'ten.webm');
 execFileSync('ffmpeg',['-hide_banner','-loglevel','error','-y','-f','lavfi','-i','testsrc2=size=160x90:rate=12:duration=40','-c:v','libvpx','-deadline','realtime',file]);
 const clip=readFileSync(file);rmSync(temp,{recursive:true,force:true});
 test.skip(!await canDecodeWebm(page,clip),'VP8 WebM не декодируется этой тестовой средой');
 const link=(await (await page.request.post(url('/api/links'),{headers:{'x-fsfy':'1'},data:{url:'https://example.com/ten-viewers.webm',label:'Десять зрителей',duration:40}})).json()).id;
 const room=(await (await page.request.post(url('/api/rooms'),{headers:{'x-fsfy':'1'},data:{linkId:link,position:8}})).json()).id;
 const contexts=[];const pages=[];const suffix=Date.now();
 try {
  for(let i=0;i<10;i++) {
   const login=`ten${suffix}${i}`,password='TenViewers123';
   expect((await page.request.post(url('/api/admin/users'),{headers:{'x-fsfy':'1'},data:{login,password,name:`Зритель ${i+1}`,role:'viewer'}})).ok()).toBe(true);
   const ctx=await browser.newContext();contexts.push(ctx);
   expect((await ctx.request.post(env().publicURL+'/api/auth/login',{headers:{'x-fsfy':'1'},data:{login,password}})).ok()).toBe(true);
   await ctx.route(`**/api/links/${link}/stream/p?*`,route=>{
    const range=/bytes=(\d+)-(\d*)/.exec(route.request().headers().range??'');
    const start=Number(range?.[1]??0),end=range?.[2]?Number(range[2]):clip.length-1;
    return route.fulfill({status:range?206:200,contentType:'video/webm',headers:{'accept-ranges':'bytes','content-length':String(end-start+1),...(range?{'content-range':`bytes ${start}-${end}/${clip.length}`}:{})},body:clip.subarray(start,end+1)});
   });
   const viewer=await ctx.newPage();viewer.on('pageerror',error=>console.error(`viewer ${i+1}: ${error.message}`));pages.push(viewer);
  }
  await Promise.all(pages.map(async p=>{
   await p.goto(env().publicURL+`/room/${room}`);
   await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.readyState),{timeout:30000}).toBeGreaterThanOrEqual(2);
   await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime)).toBeGreaterThan(6);
   await p.locator('video').evaluate(v=>{(window as any).__reloads=0;v.addEventListener('loadstart',()=>{(window as any).__reloads++})});
  }));
  await expect.poll(async()=>(await (await page.request.get(url(`/api/rooms/${room}`))).json()).members.length).toBe(10);
  await pages[0].locator('.controls .cbtn[aria-label="Смотреть"]').click();
  await Promise.all(pages.map(p=>expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(false)));
  await pages[9].locator('.controls .cbtn[aria-label="Пауза"]').click();
  await Promise.all(pages.map(async p=>{
   await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
   expect(await p.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime)).toBeGreaterThan(6);
   expect(await p.evaluate(()=>(window as any).__reloads)).toBe(0);
  }));
  await pages[0].screenshot({path:'test-results/party-ten-browser.png'});
  await pages[0].locator('.controls .cbtn[aria-label="Смотреть"]').click();
  await Promise.all(pages.map(p=>expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(false)));
  await Promise.all(contexts.map(ctx=>ctx.setOffline(true)));
  // Chromium's offline emulation leaves already-open SSE sockets alive. Leaving
  // the document closes them, so this exercises an actual all-viewers disconnect.
  await Promise.all(pages.map(p=>p.goto('about:blank')));
  await expect.poll(async()=>(await (await page.request.get(url(`/api/rooms/${room}`))).json()).members.length).toBe(0);
  const emptyState=(await (await page.request.get(url(`/api/rooms/${room}`))).json()).state;
  expect(emptyState.playing).toBe(true);
  await Promise.all(contexts.map(ctx=>ctx.setOffline(false)));
  await Promise.all(pages.map(p=>p.goto(env().publicURL+`/room/${room}`)));
  await expect.poll(async()=>(await (await page.request.get(url(`/api/rooms/${room}`))).json()).members.length,{timeout:15000}).toBe(10);
  await Promise.all(pages.map(async p=>{
   await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.readyState),{timeout:30000}).toBeGreaterThanOrEqual(2);
   await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(false);
   expect(await p.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime)).toBeGreaterThanOrEqual(emptyState.position-.5);
  }));
 } catch(error) {
  console.log(await Promise.all(pages.map(async p=>({url:p.url(),state:await p.evaluate(()=>({text:document.body.innerText.slice(0,500),videos:[...document.querySelectorAll('video')].map(v=>({ready:v.readyState,error:v.error?.code,src:v.currentSrc}))})).catch(()=>null)}))));
  throw error;
 } finally {await Promise.all(contexts.map(ctx=>ctx.close()));}
});

test('два плеера: поздний вход, пауза без перезагрузки и скрытие реакций',async({page,context,browser},info)=>{
 test.setTimeout(90000);
 const temp=mkdtempSync(join(tmpdir(),'nova-party-'));const file=join(temp,'party.webm');
 execFileSync('ffmpeg',['-hide_banner','-loglevel','error','-y','-f','lavfi','-i','testsrc2=size=160x90:rate=12:duration=60','-c:v','libvpx','-deadline','realtime',file]);
 const clip=readFileSync(file);rmSync(temp,{recursive:true,force:true});
 test.skip(!await canDecodeWebm(page,clip),'VP8 WebM не декодируется этой тестовой средой');
 const linkResponse=await page.request.post(url('/api/links'),{headers:{'x-fsfy':'1'},data:{url:'https://example.com/party-test.webm',label:'Совместный просмотр',duration:60}});
 const link=(await linkResponse.json()).id;
 const roomResponse=await page.request.post(url('/api/rooms'),{headers:{'x-fsfy':'1'},data:{linkId:link,position:12}});const room=(await roomResponse.json()).id;
 const otherContext=await browser.newContext({storageState:'e2e/.state.json',viewport:page.viewportSize()??undefined});
 let loads=0;
 for(const ctx of [context,otherContext]) await ctx.route(`**/api/links/${link}/stream/p?*`,route=>{
  loads++;const range=/bytes=(\d+)-(\d*)/.exec(route.request().headers().range??'');
  const start=Number(range?.[1]??0),end=range?.[2]?Number(range[2]):clip.length-1;
  return route.fulfill({status:range?206:200,contentType:'video/webm',headers:{'accept-ranges':'bytes','content-length':String(end-start+1),...(range?{'content-range':`bytes ${start}-${end}/${clip.length}`}:{})},body:clip.subarray(start,end+1)});
 });
 const other=await otherContext.newPage();
 try {
  await page.goto(url(`/room/${room}`));await other.goto(url(`/room/${room}`));
  for(const p of [page,other]) {
   await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.readyState)).toBeGreaterThanOrEqual(2);
   await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime)).toBeGreaterThan(10);
   expect(await p.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
   await p.locator('video').evaluate(v=>{(window as any).__partyVideo=v;(window as any).__reloads=0;v.addEventListener('loadstart',()=>{(window as any).__reloads++})});
  }
  await page.locator('.controls .cbtn[aria-label="Смотреть"], .touch-toggle[aria-label="Смотреть"]').filter({visible:true}).first().evaluate((b:HTMLButtonElement)=>b.click());
  for(const p of [page,other])await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(false);
  await other.locator('.controls .cbtn[aria-label="Пауза"], .touch-toggle[aria-label="Пауза"]').filter({visible:true}).first().evaluate((b:HTMLButtonElement)=>b.click());
  for(const p of [page,other]) {
   await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
   expect(await p.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime)).toBeGreaterThan(10);
   expect(await p.locator('video').evaluate(v=>v===(window as any).__partyVideo)).toBe(true);
  }
  for(const p of [page,other])expect(await p.evaluate(()=>(window as any).__reloads)).toBe(0);
  const position=await page.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime);
  await page.waitForTimeout(3500);
  expect(Math.abs(await page.locator('video').evaluate((v:HTMLVideoElement)=>v.currentTime)-position)).toBeLessThan(.5);
  await page.locator('.controls .cbtn[aria-label="Смотреть"], .touch-toggle[aria-label="Смотреть"]').filter({visible:true}).first().evaluate((b:HTMLButtonElement)=>b.click());
  await page.mouse.move(80,120);
  await expect(page.getByRole('group',{name:'Реакции',exact:true})).toBeHidden({timeout:6000});
  await expect(page.getByRole('complementary',{name:'Смотрим вместе'})).toBeVisible();
  const input=page.getByRole('textbox',{name:'Сообщение в комнату'});
  await input.fill('тест с пробелом');
  await input.press('Space');
  expect(await page.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(false);
  await input.blur();
  await page.keyboard.press('Space');
  for(const p of [page,other]) await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
  await page.keyboard.press('Space');
  for(const p of [page,other]) await expect.poll(()=>p.locator('video').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(false);
  await expect.poll(async()=>{const r=await page.request.get(url(`/api/links/${link}/resume`));return r.ok() ? (await r.json()).position : 0;},{timeout:10000}).toBeGreaterThan(5);
  await page.mouse.move(220,180);
  await expect(page.getByRole('group',{name:'Реакции',exact:true})).toBeVisible();
  await expect(page.getByRole('complementary',{name:'Смотрим вместе'})).toBeVisible();
  await page.screenshot({path:`test-results/party-beta9-${info.project.name}.png`});
 } finally {await otherContext.close();}
});

test('совместный просмотр создаётся из плеера с текущего места',async({page})=>{
 await page.route('**/api/links/989899',route=>route.fulfill({json:{id:989899,kind:'direct',url:'/party.mp4',label:'Фильм',progress:null}}));
 await page.route('**/party.mp4',route=>route.abort());
 let body:any;
 await page.route('**/api/rooms',route=>{body=route.request().postDataJSON();return route.fulfill({json:{id:'partyroom'}})});
 await page.route('**/api/rooms/partyroom/events',route=>route.abort());
 await page.goto(url('/watch/link/989899'));
 await page.getByRole('button',{name:'Смотреть вместе',exact:true}).click();
 await expect(page).toHaveURL(/room\/partyroom/);expect(body.linkId).toBe(989899);expect(body.position).toBeGreaterThanOrEqual(0);
});

test('быстрый источник появляется до ответа медленного',async({page})=>{
 await page.route('**/api/links/search?*',async route=>{
  const provider=new URL(route.request().url()).searchParams.get('provider');
  if(provider==='rutube')await new Promise(r=>setTimeout(r,2500));
  return route.fulfill({json:provider==='commons'?[{site:'commons',url:'https://example.com/fast.webm',label:'Быстрый фильм',thumb:null,thumbUrl:null,duration:600,author:'Wikimedia Commons'}]:[]});
 });
 await page.goto(url(`/title/${env().ids['Амели']}`));
 await page.getByRole('region',{name:'Видео по ссылке'}).getByRole('button',{name:'Добавить',exact:true}).click();
 const modal=page.getByRole('dialog',{name:'Смотреть по ссылке'});
 await expect(modal.getByRole('button',{name:/Быстрый фильм/})).toBeVisible({timeout:1500});
 await expect(modal.getByRole('status')).toContainText('Дополняем');
 await modal.getByRole('button',{name:'Закрыть',exact:true}).click();
 await expect(modal).not.toBeVisible({timeout:150});
});
