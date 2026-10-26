<script lang="ts">
  import { api } from '$lib/api';
  import { artSet } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import type Hls from 'hls.js';
  import { trailerHeight } from './trailer-quality';
  import { qualityTier } from '$lib/player/quality-label';
  import { measure, zoomFor } from './letterbox';
  // bare - без своих кнопок (на телефоне кнопку звука рисует MobileHero, как в макете);
  // available/muted - наружу, toggleSound() - через bind:this.
  let {
    imdbId,
    poster = null,
    bare = false,
    available = $bindable(false),
    muted = $bindable(true),
  }: { imdbId: string | null; poster?: string | null; bare?: boolean; available?: boolean; muted?: boolean } = $props();
  let root = $state<HTMLElement>();
  let video = $state<HTMLVideoElement>();
  let visible = $state(false);
  let playing = $state(false);
  let userPaused = false;
  // Чёрные полосы в трейлере уходят за край: наименьшая полоса по трём кадрам (1, 3 и 6 с).
  let zoom = $state(1);
  let bars: number[] = [];
  let probes: ReturnType<typeof setTimeout>[] = [];
  function resetBars() {
    probes.forEach(clearTimeout);
    probes = [];
    bars = [];
    zoom = 1;
  }
  function probeBars() {
    if (probes.length || !video) return;
    probes = [1000, 3000, 6000].map((ms) =>
      setTimeout(() => {
        const bar = video ? measure(video) : null;
        if (bar === null) return;
        bars.push(bar);
        if (bars.length >= 2) zoom = zoomFor(Math.min(...bars));
      }, ms),
    );
  }
  $effect(() => {
    if (!root) return;
    const observer = new IntersectionObserver(([entry]) => visible = entry.isIntersecting,{threshold:.25});
    observer.observe(root); return () => observer.disconnect();
  });
  // Баннер без звука - официальный трейлер IMDb в HD (у русских с сайтов картинка часто мыльная).
  // Включили звук - русский трейлер, если он есть; нет - официальный со звуком.
  let source: 'hd' | 'ru' | null = null;
  let toRussian: (() => Promise<boolean>) | null = null;
  $effect(() => {
    if (!imdbId || !video || !visible) return;
    const id=imdbId, el=video;
    let live=true, player:Hls | null=null;
    const controller=new AbortController();
    const reset=()=>{player?.destroy();player=null;el.pause();el.removeAttribute('src');el.load();};
    // Официальный HD: грузится или нет (ошибка, нет трейлера) - за 8 секунд.
    const tryHd=()=>new Promise<boolean>((resolve)=>{
      const done=(ok:boolean)=>{clearTimeout(t);el.removeEventListener('loadedmetadata',meta);el.removeEventListener('error',fail);resolve(ok);};
      const meta=()=>done(qualityTier(el.videoHeight,el.videoWidth)>=720);
      const fail=()=>done(false);
      const t=setTimeout(()=>done(false),8000);
      el.addEventListener('loadedmetadata',meta);el.addEventListener('error',fail);
      el.src=`/api/discover/title/${id}/trailer/hd`;el.load();
    });
    // Русский трейлер с сайтов: от 720p, качество закреплено.
    const russian=async():Promise<boolean>=>{
      const preview=await api<{linkId:number|null;linkIds?:number[]}>(`/api/discover/title/${id}/trailer`,{signal:controller.signal}).catch(()=>null);
      if(!live||!preview?.linkId)return false;
      let found:{url:string;hls:boolean}|null=null, playbackUrl='';
      for(const linkId of preview.linkIds ?? [preview.linkId]){
        try{
          const candidate=await api<{url:string;hls:boolean}>(`/api/links/${linkId}/stream/source`,{signal:controller.signal});
          if(!live)return false;
          let url=candidate.url;
          if(candidate.hls){
            const manifest=await fetch(url,{signal:controller.signal,credentials:'same-origin'});
            if(!manifest.ok)continue;
            const height=trailerHeight(await manifest.text());
            if(!live)return false;
            if(height===null)continue;
            url+=`${url.includes('?')?'&':'?'}height=${height}`;
          }
          found=candidate;playbackUrl=url;break;
        }catch{if(!live)return false;}
      }
      if(!found)return false;
      reset();
      if(found.hls&&!el.canPlayType('application/vnd.apple.mpegurl')){
        const {default:Hls}=await import('hls.js/light');if(!live||!Hls.isSupported())return false;
        const h=new Hls({maxBufferLength:12,backBufferLength:0,startLevel:-1});
        player=h;
        const ok=await new Promise<boolean>((resolve)=>{
          // Не ниже 720p: лучший вариант от 720p закрепляем вручную (автокачество не опустит его
          // на медленной сети); такого нет - не показываем.
          h.on(Hls.Events.MANIFEST_PARSED,()=>{
            const best=h.levels.map((l,i)=>({i,tier:qualityTier(l.height,l.width)})).filter((l)=>l.tier>=720&&l.tier<=1080).sort((x,y)=>y.tier-x.tier)[0];
            if(!best)return resolve(false);
            h.currentLevel=best.i;resolve(true);
          });
          h.on(Hls.Events.ERROR,(_,data)=>{if(data.fatal)resolve(false);});
          h.loadSource(playbackUrl);h.attachMedia(el);
        });
        if(!ok){reset();return false;}
      }else{el.src=playbackUrl;}
      source='ru';
      return true;
    };
    toRussian=async()=>{const ok=await russian();if(ok&&live&&!userPaused)void el.play().catch(()=>{el.muted=true;muted=true;void el.play().catch(()=>{});});return ok;};
    const timer=setTimeout(async()=>{
      try {
        el.muted=true;muted=true;
        if(await tryHd()){
          if(!live)return;
          // Официальные трейлеры начинаются с чёрной заставки - баннер сразу со сцены.
          if(isFinite(el.duration)&&el.duration>30)el.currentTime=Math.min(10,el.duration*0.08);
          source='hd';available=true;if(!userPaused)void el.play().catch(()=>{});
          return;
        }
        if(!live)return;
        reset();
        // Сам без звука - только официальный: русские с сайтов - чужие перезаливки (вшитые «16+»,
        // полосы), по кадру не понять, что это трейлер. Они - по кнопке и сразу со звуком.
        const preview=await api<{linkId:number|null}>(`/api/discover/title/${id}/trailer`,{signal:controller.signal}).catch(()=>null);
        if(live&&preview?.linkId){source=null;available=true;}
      }catch{ /* Баннер остаётся на кадре, если трейлер недоступен. */ }
    },1200);
    const visibility=()=>{if(document.hidden)el.pause();else if(live&&available&&!userPaused)void el.play().catch(()=>{});};
    document.addEventListener('visibilitychange',visibility);
    return()=>{live=false;toRussian=null;source=null;clearTimeout(timer);controller.abort();reset();playing=false;available=false;document.removeEventListener('visibilitychange',visibility);};
  });

  // Официального нет, русский ещё не загружен - по нажатию и со звуком.
  async function startRussian() {
    if (!video || !toRussian) return;
    muted = false;
    video.muted = false;
    userPaused = false;
    if (!(await toRussian())) available = false;
  }

  function togglePlay() {
    if (!video) return;
    if (!source) return void startRussian();
    userPaused = !video.paused;
    if (video.paused) void video.play().catch(() => {});
    else video.pause();
  }

  // Звук: у официального HD - сначала попробовать русский трейлер.
  export async function toggleSound() {
    if (!video) return;
    if (!source) return startRussian();
    if (muted && source === 'hd' && toRussian) {
      muted = false;
      video.muted = false;
      const switched = await toRussian();
      if (!switched && video) {
        // Русского нет - официальный со звуком (заново: reset() снял его).
        video.src = `/api/discover/title/${imdbId}/trailer/hd`;
        source = 'hd';
        if (!userPaused) void video.play().catch(() => {});
      }
      if (video) video.muted = false;
      return;
    }
    muted = !muted;
    video.muted = muted;
  }
</script>
<section class="trailer" class:bare bind:this={root} aria-label="Баннер и трейлер">
  {#if poster}<img src={poster} srcset={artSet(poster)} sizes="100vw" alt="" class:dim={playing} decoding="async" />{/if}
  <!-- svelte-ignore a11y_media_has_caption -->
  <video bind:this={video} class:ready={playing} muted playsinline loop preload="none"
    onloadedmetadata={()=>{if(video&&qualityTier(video.videoHeight,video.videoWidth)<720){video.pause();video.removeAttribute('src');available=false;playing=false;}}}
    onloadstart={resetBars} onplaying={()=>{playing=true;probeBars();}} style:transform={zoom===1?undefined:`scale(${zoom})`} onpause={()=>playing=false}></video>
  <div class="shade"></div>
  {#if available && !bare}<div class="controls"><span>Трейлер</span><button aria-label={playing?'Приостановить трейлер':'Включить трейлер'} onclick={togglePlay}><Icon name={playing?'pause':'play'} size={17}/></button><button aria-label={muted?'Включить звук трейлера':'Выключить звук трейлера'} onclick={toggleSound}><Icon name={muted?'mute':'volume'} size={18}/></button></div>{/if}
</section>
<style>
  .trailer{position:absolute;inset:0;overflow:hidden;background:#000}.trailer.bare{background:transparent}img,video{position:absolute;inset:0;width:100%;height:100%;object-fit:cover;object-position:center 30%}video{opacity:0;transition:opacity .45s ease,transform .6s ease}video.ready{opacity:1}img{transition:opacity .45s ease}.dim{opacity:0}.shade{position:absolute;inset:0;background:linear-gradient(0deg,#000,transparent 30%,#0002);pointer-events:none}.controls{position:absolute;right:var(--gutter);bottom:28px;display:flex;align-items:center;gap:8px;color:#eee}.controls span{font-size:11px;letter-spacing:.08em;margin-right:8px}.controls button{width:38px;height:38px;border:1px solid #ffffff60;border-radius:50%;background:#000b;color:#fff;display:grid;place-items:center;cursor:pointer;transition:background .2s,transform .2s}.controls button:hover{background:#000;transform:scale(1.04)}
</style>
