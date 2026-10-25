<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, errorText, post } from '$lib/api';
  import Player from '$lib/player/Player.svelte';
  import type { VideoLink } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';

  // Видео по ссылке - в плеере Nova: Rutube, VK Видео и свои сайты хоста - поток через сервер
  // (сайты браузеру его напрямую не дают), прямые .mp4/.webm/.m3u8 - как есть. Не открылось -
  // «Плеер сайта» (встроенный плеер Rutube/VK или страница своего сайта). YouTube - только своим.
  const id = $derived(Number(page.params.id));
  let link = $state<VideoLink | null>(null);
  let src = $state<{ url: string; hls: boolean; title: string; linkId: number; start: number } | null>(null);
  let own = $state(false);
  // Другие видео этого фильма - «Озвучка» в плеере.
  let alts = $state<{ id: number; label: string }[]>([]);
  let error = $state('');
  let nextSource = $state<{id:number;title:string}|null>(null);
  let nextLoading = $state(false), nextChecked = false, finished = $state(false);
  function prepareNext() {
    if(nextChecked || link?.season == null || link?.episode == null)return;
    nextChecked=true;nextLoading=true;const want=id;
    api<{next:{id:number;title:string}|null}>(`/api/links/${want}/next`).then(r=>{if(want===id)nextSource=r.next;},()=>{}).finally(()=>{if(want===id)nextLoading=false;});
  }

  $effect(() => {
    const want = id;
    link = null;
    nextSource=null;nextLoading=false;nextChecked=false;finished=false;
    own = false;
    alts = [];
    error = '';
    api<VideoLink>(`/api/links/${want}`).then(
      async (l) => {
        if (want !== id) return;
        link = l;
        const q = l.titleId ? `titleId=${l.titleId}` : l.extId ? `extId=${encodeURIComponent(l.extId)}` : '';
        if (q)
          api<VideoLink[]>(`/api/links?${q}`).then(
            (all) => want === id && (alts = all.filter((a, index) => all.findIndex(b => b.url === a.url) === index).filter((a) => a.kind !== 'youtube' && a.season === l.season && a.episode === l.episode).map((a) => ({ id: a.id, label: a.label }))),
            () => {},
          );
        if (l.kind === 'youtube') { src=null;own=true;return; }
        // С места, где остановился (досмотрел почти до конца - с начала).
        // Место - последнее в этом фильме (серии) по любому видео: на другом устройстве могли
        // смотреть другой источник.
        const r = await api<{ position: number | null; duration: number | null }>(`/api/links/${want}/resume`).catch(() => null);
        const p = r?.position != null && r.duration ? { position: r.position, duration: r.duration } : l.progress;
        const start = p && p.position < p.duration * 0.95 ? p.position : 0;
        if (l.kind === 'direct') return void (src = { url: l.url, hls: /\.m3u8(\?|$)/i.test(l.url), title: l.label, linkId: l.id, start });
        try {
          const s = await api<{ url: string; hls: boolean }>(`/api/links/${want}/stream/source`);
          if (want === id) src = { ...s, title: l.label, linkId: l.id, start };
        } catch (x) {
          if (want !== id) return;
          // Сайт не отдал поток - его собственный плеер (у своего сайта - сообщение и ссылка).
          src=null;error = errorText(x);
        }
      },
      (e) => {src=null;error=errorText(e);},
    );
  });

  // «Это не тот фильм»: источник - в чёрный список поиска, дальше - следующее видео этого фильма,
  // а если других нет - к фильму (там поиск видео начнётся заново, уже без этого).
  // Длительность фильма - чтобы плеер заметил, если видео явно другое.
  let expected = $state<number | null>(null);
  $effect(() => {
    const l = link;
    expected = null;
    if (!l) return;
    const url = l.extId ? `/api/discover/title/${l.extId}` : l.titleId ? `/api/titles/${l.titleId}` : null;
    if (!url || l.season != null) return;
    api<{ runtime?: number | null; card?: { runtime?: number | null } }>(url).then(
      (d) => {
        const min = d.runtime ?? d.card?.runtime ?? null;
        if (link === l) expected = min ? min * 60 : null;
      },
      () => {},
    );
  });

  async function wrong() {
    const l = link;
    if (!l) return;
    await post(`/api/links/${l.id}/wrong`).catch(() => {});
    const next = alts.find((a) => a.id !== l.id);
    if (next) goto(`/watch/link/${next.id}`, { replaceState: true });
    else goto(l.extId ? `/ext/${l.extId}` : l.titleId ? `/title/${l.titleId}` : '/discover', { replaceState: true });
  }

  function fallback() {
    if (link?.embed) {
      src = null;
      own = true;
    } else if (link) window.open(link.url, '_blank', 'noopener');
  }

  function back() {
    if (history.length > 1) history.back();
    else goto('/discover');
  }

  // Esc - назад в сервис, пока плеер Nova не открылся (готовим видео, ошибка, плеер сайта: фокус внутри него Esc сюда не отдаёт -
  // для этого кнопка «Назад»).
  $effect(() => {
    if (src) return; // в плеере Esc - его
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || document.fullscreenElement) return;
      e.preventDefault();
      back();
    };
    addEventListener('keydown', key);
    return () => removeEventListener('keydown', key);
  });
</script>

{#if src}
  <Player {src} {alts} {nextSource} onnext={()=>{if(nextSource)void goto(`/watch/link/${nextSource.id}`,{replaceState:true});}} onNearEnd={prepareNext} onfinish={()=>{finished=true;prepareNext();}} onalt={(a) => goto(`/watch/link/${a}`, { replaceState: true })} onwrong={wrong} expect={expected} onfail={fallback}>
    {#if finished && !nextSource && link?.season != null}<div class="next-status" role="status">{nextLoading ? 'Ищем следующую серию…' : 'Источник следующей серии не найден'}{#if !nextLoading}<button class="btn sm" onclick={back}>Выбрать серию</button>{/if}</div>{/if}
  </Player>
{:else}
  <div class="stage">
    {#if own && link?.embed}
      <iframe
        src={link.embed}
        title={link.label}
        allow="autoplay; fullscreen; picture-in-picture; encrypted-media; clipboard-write"
        allowfullscreen
        referrerpolicy="strict-origin-when-cross-origin"
      ></iframe>
    {/if}
    {#if error}
      <div class="msg">
        <Icon name="offline" size={36} />
        <p>{error}</p>
        <div class="row">
          <button class="btn" onclick={() => { error = ''; location.reload(); }}><Icon name="refresh" size={16} /> Повторить</button><button class="btn" onclick={back}>Выбрать источник</button>
          {#if link}<a class="btn white" href={link.url} target="_blank" rel="noopener noreferrer"><Icon name="globe" size={16} /> Открыть на сайте</a>{/if}
        </div>
      </div>
    {:else if !own}
      <div class="msg"><span class="spin" aria-label="Загрузка"></span><p class="muted">Готовим видео с сайта…</p></div>
    {/if}
    <div class="bar">
      <button class="round" onclick={back} aria-label="Назад" title="Назад (Esc)"><Icon name="back" size={22} /></button>
      {#if link && !own}<span class="name">{link.label}</span>{/if}
    </div>
  </div>
{/if}

<style>
  .next-status{position:absolute;right:24px;bottom:130px;z-index:5;display:flex;gap:16px;align-items:center;padding:16px;border:1px solid #333;border-radius:16px;background:#000;color:#fff;max-width:calc(100% - 48px);font-size:13px;}
  .stage {
    position: fixed;
    inset: 0;
    background: #000;
    display: grid;
  }
  iframe {
    grid-area: 1 / 1;
    width: 100%;
    height: 100%;
    border: 0;
    background: #000;
  }
  .bar {
    position: absolute;
    top: calc(12px + env(safe-area-inset-top));
    left: 12px;
    right: 12px;
    display: flex;
    align-items: center;
    gap: 12px;
    pointer-events: none;
  }
  .round {
    pointer-events: auto;
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    flex: none;
    border: 0;
    border-radius: 50%;
    background: rgba(30, 30, 32, 0.85);
    color: #fff;
    cursor: pointer;
  }
  .round:hover {
    background: rgba(60, 60, 64, 0.95);
  }
  .name {
    min-width: 0;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-shadow: 0 1px 6px rgba(0, 0, 0, 0.9);
  }
  .msg {
    grid-area: 1 / 1;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
    text-align: center;
    padding: 24px;
  }
  .msg p {
    margin: 0;
    max-width: 46ch;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
  }
  .spin {
    width: 32px;
    height: 32px;
    border: 3px solid rgba(255, 255, 255, 0.2);
    border-top-color: #fff;
    border-radius: 50%;
    animation: turn 0.8s linear infinite;
  }
  @keyframes turn {
    to {
      transform: rotate(1turn);
    }
  }
</style>
