import type { Page } from '@playwright/test';

// Windows WebKit can report "probably" for VP8 yet fail an independent valid clip
// with MEDIA_ERR_SRC_NOT_SUPPORTED. Probe the decoder, not the application source.
export async function canDecodeWebm(page: Page, clip: Buffer): Promise<boolean> {
 return page.evaluate(base64=>new Promise<boolean>(resolve=>{
  const video=document.createElement('video');video.muted=true;video.preload='auto';
  let finished=false;
  const done=(ok:boolean)=>{if(finished)return;finished=true;clearTimeout(timer);video.onloadeddata=null;video.onerror=null;video.removeAttribute('src');video.load();resolve(ok)};
  const timer=setTimeout(()=>done(false),3000);
  video.onloadeddata=()=>done(true);video.onerror=()=>done(false);
  video.src='data:video/webm;base64,'+base64;video.load();
 }),clip.toString('base64'));
}
