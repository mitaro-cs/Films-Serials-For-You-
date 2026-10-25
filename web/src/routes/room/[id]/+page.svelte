<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, errorText, post } from '$lib/api';
  import Player, { type PlayerSync } from '$lib/player/Player.svelte';
  import { session } from '$lib/session.svelte';
  import type { RoomChat, RoomEvent, RoomState, Who } from '$lib/types';
  import { Voice } from '$lib/room/voice.svelte';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';

  // Смотреть вместе: у всех в комнате одно место в видео - пауза, перемотка и следующая серия у
  // всех сразу. Сбоку - кто в комнате и чат, внизу - реакции, летящие вверх по экрану. Связь -
  // поток событий сервера (EventSource), действия - обычные запросы.
  const id = $derived(page.params.id ?? '');
  const REACTIONS = ['❤️', '😂', '😮', '😢', '🔥', '👏', '😱', '👍'];

  let room = $state<RoomState | null>(null);
  let receivedAt = 0;
  let titleId = $state(0);
  let members = $state<Who[]>([]);
  let chat = $state<RoomChat[]>([]);
  let error = $state('');
  let side = $state(false);
  let connection = $state('connecting');
  let text = $state('');
  let inviting = $state(false);
  let friends = $state<Who[] | null>(null);
  let invited = $state<number[]>([]);
  let flying = $state<{ key: number; emoji: string; x: number; who: string }[]>([]);
  let feed = $state<HTMLElement>();
  // Видео по ссылке (фильм из Обзора): поток через сервер - узнаём один раз.
  let linkSrc = $state<{ url: string; hls: boolean; title: string; linkId: number; start: number } | null>(null);
  $effect(() => {
    const l = room?.linkId;
    if (!l || linkSrc?.linkId === l) return;
    Promise.all([api<{ url: string; hls: boolean }>(`/api/links/${l}/stream/source`), api<{ label: string }>(`/api/links/${l}`)]).then(
      ([s, link]) => (linkSrc = { ...s, title: link.label, linkId: l, start: room ? expected(room).position : 0 }),
      (x) => (error = errorText(x)),
    );
  });
  // Голосовой чат - заводится, когда пришёл снимок комнаты.
  let voice = $state<Voice | null>(null);

  const listeners = new Set<(s: { playing: boolean; position: number }) => void>();
  /** Где сейчас должно быть видео: при «смотрим» место идёт дальше с момента события. */
  const expected = (s: RoomState) => ({
    playing: s.playing,
    position: s.playing ? s.position + (Date.now() - receivedAt) / 1000 : s.position,
  });

  let acceptedRevision = -1;
  let sending = false;
  let queued: {fileId:number;playing:boolean;position:number} | null = null;
  let deferred: RoomState | null = null;
  function setState(s: RoomState) {
    if((s.revision??0) < acceptedRevision)return;
    acceptedRevision=s.revision??0;
    room = s;
    receivedAt = Date.now();
    const e = expected(s);
    listeners.forEach((l) => l(e));
  }

  let key = 0;
  function fly(emoji: string, who: string) {
    const k = ++key;
    flying = [...flying, { key: k, emoji, x: 10 + Math.random() * 70, who }];
    setTimeout(() => (flying = flying.filter((f) => f.key !== k)), 2600);
  }

  function scrollChat() {
    requestAnimationFrame(() => feed?.scrollTo({ top: feed.scrollHeight, behavior: 'smooth' }));
  }

  $effect(() => {
    const room_ = id;
    error = '';
    const es = new EventSource(`/api/rooms/${room_}/events`);
    es.onmessage = (m) => {
      connection = 'online';
      const e = JSON.parse(m.data) as RoomEvent;
      if (e.type === 'snapshot') {
        const me = session.me?.user.profile?.id;
        if (me && !voice) voice = new Voice(room_, me);
        voice?.setIds(e.voice ?? []);
        titleId = e.titleId;
        members = e.members;
        chat = e.chat;
        setState(e.state);
        scrollChat();
      } else if (e.type === 'state') {if(sending)deferred=e.state;else setState(e.state);}
      else if (e.type === 'members') members = e.members;
      else if (e.type === 'chat') {
        chat = [...chat.slice(-59), { from: e.from, body: e.body, at: e.at }];
        scrollChat();
      } else if (e.type === 'react') fly(e.emoji, e.from.name);
      else if (e.type === 'voice') voice?.setIds(e.ids);
      else if (e.type === 'signal' && e.to === session.me?.user.profile?.id) voice?.signal(e.from, e.data);
    };
    // Обрыв: EventSource переподключается сам; комнаты больше нет - сказать и не стучаться.
    es.onerror = () => {
      connection = 'reconnecting';
      api('/api/rooms/'+room_).catch((x) => {
        if (x?.status === 404 || x?.status === 401) { error=errorText(x);es.close(); }
      });
    };
    return () => {
      es.close();
      voice?.leave();
      voice = null;
    };
  });

  // Раз в 5 с, пока смотрим, - сверить место (плеер перематывает только при заметном разрыве).
  $effect(() => {
    const t = setInterval(() => {
      if (room?.playing && !sending) listeners.forEach((l) => l(expected(room!)));
    }, 5000);
    return () => clearInterval(t);
  });

  async function flushState() {
    if(sending)return;sending=true;
    try {
      while(queued){const command=queued;queued=null;
        try {const state=await post<RoomState>(`/api/rooms/${id}/state`,command);if(!queued){const latest=deferred && (deferred.revision??0)>(state.revision??0)?deferred:state;setState(latest);deferred=null;}}
        catch {connection='reconnecting';}
      }
    } finally {sending=false;}
  }

  const sync: PlayerSync = {
    local(s) {
      if (!room) return;
      const next: RoomState = { ...room, fileId: s.fileId ?? room.fileId, playing: s.playing, position: s.position, at: Date.now() };
      room = next;
      receivedAt = Date.now();
      queued={fileId:next.fileId,playing:s.playing,position:s.position};
      void flushState();
    },
    listen(apply) {
      listeners.add(apply);
      if (room) apply(expected(room));
      return () => listeners.delete(apply);
    },
    back() {
      goto(titleId ? `/title/${titleId}` : '/');
    },
  };

  async function say(e: SubmitEvent) {
    e.preventDefault();
    const body = text.trim();
    if (!body) return;
    text = '';
    await post(`/api/rooms/${id}/chat`, { body }).catch((x) => (error = errorText(x)));
  }

  function react(emoji: string) {
    post(`/api/rooms/${id}/react`, { emoji }).catch(() => {});
  }

  async function openInvite() {
    inviting = !inviting;
    if (!inviting || friends) return;
    const r = await api<{ items: { person: Who }[] }>('/api/chats').catch(() => ({ items: [] }));
    friends = r.items.map((i) => i.person);
  }

  async function invite(p: Who) {
    try {
      await post(`/api/rooms/${id}/invite`, { profileId: p.id });
      invited = [...invited, p.id];
    } catch (x) {
      error = errorText(x);
    }
  }

  let copied = $state(false);
  // Выйти из комнаты и смотреть одному - с того же места (место сохраняем, чтобы плеер начал отсюда).
  let leaving = $state(false);
  async function alone() {
    if (!room || leaving) return;
    leaving = true;
    const v = document.querySelector<HTMLVideoElement>('.player video');
    const position = v && isFinite(v.currentTime) ? v.currentTime : expected(room).position;
    const duration = v && isFinite(v.duration) && v.duration > 0 ? v.duration : 0;
    const url = room.linkId ? `/api/links/${room.linkId}/progress` : `/api/progress/${room.fileId}`;
    if (duration && position > 5) await api(url, { method: 'PUT', json: { position, duration } }).catch(() => {});
    goto(room.linkId ? `/watch/link/${room.linkId}` : `/watch/${room.fileId}`, { replaceState: true });
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(location.href);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      /* без буфера обмена - ссылка в адресной строке */
    }
  }
  const me = $derived(session.me?.user.profile?.id);
</script>

{#if error && !room}
  <div class="page empty">
    <Icon name="party" size={40} />
    <p>{error}</p>
    <a class="btn" href="/">На главную</a>
  </div>
{:else if room}
  {#key id}
    {#if room.linkId && !linkSrc}
      <div class="boot" aria-busy="true"><Icon name="party" size={36} /> Готовим видео с сайта…</div>
    {:else}
    <Player fileId={room.fileId} src={linkSrc} {sync}>
      <div class="room" class:closed={!side}>
        <div class="room-status" role="status"><Icon name="party" size={14}/><span>Смотрим вместе</span><i class:online={connection === 'online'}></i><small>{connection === 'online' ? `${members.length} участн.` : 'Восстанавливаем связь…'}</small></div>
        <button class="toggle" aria-label={side ? 'Спрятать чат' : 'Показать чат'} aria-expanded={side} onclick={() => (side = !side)}>
          <Icon name={side ? 'right' : 'mail'} size={18} />
          {#if !side && members.length > 1}<i>{members.length}</i>{/if}
        </button>
        {#if side}
          <aside class="side" aria-label="Смотрим вместе">
            <header>
              <b><Icon name="party" size={16} /> Смотрим вместе</b>
              <span class="faces">
                {#each members.slice(0,4) as m, i (m.id)}
                  <span class="face" class:talk={voice?.speaking[m.id]} class:mic={voice?.ids.includes(m.id)} style:--i={i} title="{m.name}{voice?.ids.includes(m.id) ? ' · в голосовом чате' : ''}"><Avatar avatar={m.avatar} photo={m.photo} size={26} /></span>
                {/each}
                {#if members.length>4}<span class="extra-members">+{members.length-4}</span>{/if}
              </span>
            </header>
            <div class="tools">
              {#if voice?.on}
                <button class="btn sm" class:off={voice.muted} aria-pressed={!voice.muted} onclick={() => voice?.toggleMute()} title={voice.muted ? 'Включить микрофон' : 'Выключить микрофон'}>
                  <Icon name={voice.muted ? 'mute' : 'audio'} size={14} /> {voice.muted ? 'Микрофон выкл.' : 'Говорю'}
                </button>
                <button class="btn sm ghost" onclick={() => voice?.leave()}><Icon name="close" size={14} /> Выйти из голоса</button>
              {:else}
                <button class="btn sm white" disabled={!voice || voice.busy} onclick={() => voice?.join()}><Icon name="audio" size={14} /> Голос{voice?.ids.length ? ` · ${voice.ids.length}` : ''}</button>
              {/if}
              <button class="btn sm" aria-expanded={inviting} onclick={openInvite}><Icon name="plus" size={14} /> Позвать</button>
              <button class="btn sm ghost" disabled={leaving} onclick={alone}><Icon name="user" size={14} /> Смотреть одному</button>
              <button class="btn sm ghost" onclick={copy}><Icon name={copied ? 'check' : 'link'} size={14} /> {copied ? 'Скопировано' : 'Ссылка'}</button>
            </div>
            {#if inviting}
              <ul class="invite">
                {#if friends === null}
                  <li class="muted">Загружаем друзей…</li>
                {:else if !friends.length}
                  <li class="muted">Друзей пока нет - отправьте ссылку.</li>
                {/if}
                {#each friends ?? [] as f (f.id)}
                  <li>
                    <Avatar avatar={f.avatar} photo={f.photo} size={24} />
                    <span>{f.name}</span>
                    <button class="btn sm" disabled={invited.includes(f.id) || members.some((m) => m.id === f.id)} onclick={() => invite(f)}>
                      {members.some((m) => m.id === f.id) ? 'Тут' : invited.includes(f.id) ? 'Позвали' : 'Позвать'}
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
            <ol class="feed" bind:this={feed} aria-live="polite">
              {#if room.by}<li class="sys">Последним управлял(а) {room.by}</li>{/if}
              {#each chat as m, i (i)}
                <li class="msg" class:mine={m.from.id === me}>
                  <Avatar avatar={m.from.avatar} photo={m.from.photo} size={22} />
                  <span><b>{m.from.name}</b> {m.body}</span>
                </li>
              {/each}
            </ol>
            {#if voice?.error}<p class="error">{voice.error}</p>{/if}
            {#if error}<p class="error">{error}</p>{/if}
            <form class="say" onsubmit={say}>
              <input class="input" bind:value={text} maxlength="500" placeholder="Сообщение…" aria-label="Сообщение в комнату" />
              <button class="btn sm icon" aria-label="Отправить" disabled={!text.trim()}><Icon name="send" size={16} /></button>
            </form>
          </aside>
        {/if}
        <div class="reacts" role="group" aria-label="Реакции">
          {#each REACTIONS as r (r)}<button onclick={() => react(r)} aria-label="Реакция {r}">{r}</button>{/each}
        </div>
        {#each flying as f (f.key)}
          <span class="fly" style:left="{f.x}%" aria-hidden="true"><em>{f.emoji}</em><small>{f.who}</small></span>
        {/each}
      </div>
    </Player>
    {/if}
  {/key}
{:else}
  <div class="boot" aria-busy="true"><Icon name="party" size={36} /> Входим в комнату…</div>
{/if}

<style>
  .boot {
    min-height: 100dvh;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
    color: var(--muted);
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.5;
    }
  }
  /* Слой комнаты поверх видео: сам не ловит касания, только свои кнопки и панель. */
  .room {
    position: absolute;
    inset: 0;
    z-index: 6;
    pointer-events: none;
  }
  .room > :global(*:not(.fly)) {
    pointer-events: auto;
  }
  .toggle {
    position: absolute;
    top: 76px;
    right: 16px;
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border: 0;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
    cursor: pointer;
    transition: transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.5);
  }
  .toggle:active {
    transform: scale(0.88);
  }
  .toggle i {
    position: absolute;
    top: -4px;
    right: -4px;
    min-width: 18px;
    height: 18px;
    padding: 0 4px;
    border-radius: 99px;
    background: var(--ok);
    color: #000;
    font-size: 11px;
    font-style: normal;
    font-weight: 800;
    line-height: 18px;
  }
  .side {
    position: absolute;
    top: 124px;
    right: 16px;
    bottom: 120px;
    width: min(320px, calc(100vw - 32px));
    display: grid;
    grid-template-rows: auto auto auto minmax(0, 1fr) auto auto;
    gap: 10px;
    padding: 14px;
    border-radius: 18px;
    background: rgba(14, 14, 16, 0.86);
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.08),
      0 30px 60px -20px rgba(0, 0, 0, 0.8);
    animation: slide 0.35s cubic-bezier(0.2, 0.9, 0.3, 1.15);
  }
  @keyframes slide {
    from {
      opacity: 0;
      transform: translateX(24px);
    }
  }
  .side header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .side header b {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 14px;
  }
  .faces {
    display: flex;
  }
  .face {
    display: block;
    border-radius: 50%;
    box-shadow: 0 0 0 2px #0e0e10;
    animation: pop 0.4s cubic-bezier(0.2, 0.9, 0.3, 1.6) both;
    animation-delay: calc(var(--i) * 60ms);
  }
  .face:not(:first-child) {
    margin-left: -7px;
  }
  /* В голосовом чате - зелёная точка, говорит - зелёное кольцо пульсирует. */
  .face {
    position: relative;
    transition: transform 0.2s;
  }
  .face.mic::after {
    content: '';
    position: absolute;
    right: -1px;
    bottom: -1px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 2px #0e0e10;
  }
  .face.talk {
    z-index: 1;
    box-shadow:
      0 0 0 2px #0e0e10,
      0 0 0 4px var(--ok);
    transform: scale(1.12);
  }
  .tools {
    flex-wrap: wrap;
  }
  .tools .off {
    color: var(--danger);
  }
  @keyframes pop {
    from {
      transform: scale(0);
    }
  }
  .tools {
    display: flex;
    gap: 6px;
  }
  .invite {
    list-style: none;
    margin: 0;
    padding: 6px;
    max-height: 180px;
    overflow-y: auto;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.06);
    display: grid;
    gap: 4px;
  }
  .invite li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .invite li span {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .feed {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    display: grid;
    align-content: start;
    gap: 8px;
    font-size: 13.5px;
  }
  .sys {
    color: var(--faint);
    font-size: 12px;
    text-align: center;
  }
  .msg {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    animation: in 0.3s cubic-bezier(0.2, 0.9, 0.3, 1.2);
  }
  .msg span {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .msg b {
    font-weight: 700;
    margin-right: 4px;
  }
  .msg.mine b {
    color: var(--ok);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
  .say {
    display: flex;
    gap: 6px;
  }
  .say .input {
    height: 36px;
    min-width: 0;
  }
  .error {
    margin: 0;
    font-size: 12.5px;
  }
  /* Реакции - у нижнего края, над управлением плеера. */
  .reacts {
    position: absolute;
    left: 50%;
    bottom: 96px;
    display: flex;
    gap: 4px;
    padding: 5px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.55);
    transform: translateX(-50%);
  }
  .reacts button {
    width: 36px;
    height: 36px;
    border: 0;
    border-radius: 50%;
    background: none;
    font-size: 20px;
    cursor: pointer;
    transition: transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.6);
  }
  .reacts button:hover {
    transform: translateY(-4px) scale(1.2);
  }
  .reacts button:active {
    transform: scale(0.85);
  }
  .fly {
    position: absolute;
    bottom: 140px;
    display: grid;
    justify-items: center;
    pointer-events: none;
    animation: up 2.6s cubic-bezier(0.2, 0.7, 0.3, 1) forwards;
  }
  .fly em {
    font-size: 40px;
    font-style: normal;
    line-height: 1;
  }
  .fly small {
    padding: 1px 8px;
    border-radius: 99px;
    background: rgba(0, 0, 0, 0.6);
    font-size: 11px;
    color: #fff;
  }
  @keyframes up {
    0% {
      opacity: 0;
      transform: translateY(20px) scale(0.6);
    }
    12% {
      opacity: 1;
      transform: translateY(0) scale(1.15);
    }
    100% {
      opacity: 0;
      transform: translateY(-46vh) scale(0.9) rotate(-12deg);
    }
  }
  /* В плеере управление прячется - комната тоже уходит из-под пальца, кроме реакций. */
  :global(.player.idle) .room .side,
  :global(.player.idle) .room .toggle,
  :global(.player.idle) .reacts {
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition: opacity 0.4s;
  }
  :global(.player.idle) .room .side:hover {
    opacity: 1;
  }
  @media (max-width: 640px) {
    .side {
      top: auto;
      bottom: 150px;
      height: 46vh;
    }
    .toggle {
      top: 70px;
    }
    .reacts {
      bottom: 92px;
      max-width: calc(100vw - 24px);
      overflow-x: auto;
    }
  }

  .side { background: linear-gradient(145deg, #1c2029fa, #11141afa); border: 1px solid #bbc9eb33; border-radius: 22px; padding: 18px; width: min(360px, calc(100vw - 32px)); }
  .side header { padding-bottom: 10px; border-bottom: 1px solid var(--line); }
  .toggle { border: 1px solid var(--line); background: #1c2029; }

  .room-status {position:absolute;top:84px;left:clamp(14px,3vw,36px);display:flex;align-items:center;gap:8px;padding:8px 12px;border:1px solid #ffffff12;border-radius:999px;background:#101012b8;backdrop-filter:blur(12px);color:#d7d7dc;font-size:12px;pointer-events:none;}
  .room-status small {color:#a5a5ad}.room-status i {width:6px;height:6px;border-radius:50%;background:#b9994f}.room-status i.online {background:#46c992}
  .side {top:132px;bottom:132px;border:1px solid #ffffff12;background:#111114ed;backdrop-filter:blur(18px);border-radius:24px;box-shadow:0 18px 70px #0008;animation-duration:140ms;}
  .reacts {padding:5px 10px;gap:2px;border:1px solid #ffffff10;background:#111114b8;backdrop-filter:blur(12px);border-radius:999px;transition:opacity 150ms;}
  .reacts button {width:36px;height:36px;border-radius:50%;transition:background 100ms;}
  :global(.player.idle) .room-status {opacity:0;visibility:hidden;}
  @media(max-width:640px) {.room-status {top:78px;font-size:11px}.room-status small {font-size:10px}.side {top:130px;bottom:145px;right:12px;width:calc(100vw - 24px);border-radius:20px}.reacts button {width:30px;height:36px}.reacts {max-width:calc(100vw - 24px);}}
  .extra-members {font-size:11px;color:#b8b8c0;padding-left:4px;}
  @media(max-height:500px) {.side {top:76px;bottom:100px;overflow:auto;grid-template-rows:auto auto minmax(50px,1fr) auto;}.room-status {top:68px;}.reacts {bottom:94px;}}
  /* Поверх видео - строго между панелями плеера: их высоту плеер отдаёт в --top-h и --bottom-h. */
  .room-status {
    top: calc(var(--top-h, 80px) + 8px);
    left: clamp(12px, 3vw, 36px);
    max-width: calc(100% - 92px);
    overflow: hidden;
    white-space: nowrap;
    transition: opacity 0.3s ease, transform 0.3s ease;
  }
  .room-status small {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .toggle {
    top: calc(var(--top-h, 80px) + 4px);
    right: clamp(12px, 3vw, 36px);
    transition: opacity 0.3s ease, transform 0.3s ease;
  }
  .side {
    top: calc(var(--top-h, 80px) + 56px);
    bottom: calc(var(--bottom-h, 120px) + 64px);
    height: auto;
    animation: side-in 0.25s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .reacts {
    bottom: calc(var(--bottom-h, 120px) + 10px);
    transition: opacity 0.3s ease, transform 0.3s ease;
  }
  .fly {
    bottom: calc(var(--bottom-h, 120px) + 64px);
  }
  :global(.player.idle) .room-status,
  :global(.player.idle) .room .toggle {
    opacity: 0;
    visibility: hidden;
    transform: translateY(-8px);
    transition: opacity 0.3s ease, transform 0.3s ease, visibility 0s 0.3s;
  }
  :global(.player.idle) .reacts {
    transform: translate(-50%, 8px);
    transition: opacity 0.3s ease, transform 0.3s ease, visibility 0s 0.3s;
  }
  @keyframes side-in {
    from {
      opacity: 0;
      transform: translateX(16px);
    }
  }
  @media (max-width: 640px) {
    .side {
      top: calc(var(--top-h, 80px) + 52px);
      bottom: calc(var(--bottom-h, 120px) + 60px);
      height: auto;
    }
  }
</style>
