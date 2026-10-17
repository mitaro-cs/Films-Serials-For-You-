<script lang="ts">
  import { page } from '$app/state';
  import { api, errorText, post } from '$lib/api';
  import { refreshBadges } from '$lib/social/badges.svelte';
  import ChatList from '$lib/social/ChatList.svelte';
  import type { Card, Message, Person } from '$lib/types';
  import { placeholder } from '$lib/ui/art';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Modal from '$lib/ui/Modal.svelte';

  // Переписка с другом: новые сообщения — опросом раз в 3 с (пока вкладка видна), свои —
  // сразу на экране. Можно приложить фильм — придёт карточкой.
  const id = $derived(Number(page.params.id));
  let who = $state<Person | null>(null);
  let items = $state<(Message & { pending?: boolean })[]>([]);
  let error = $state('');
  let text = $state('');
  let more = $state(true);
  let scroller = $state<HTMLDivElement>();
  let picking = $state(false);
  let q = $state('');
  let found = $state<Card[]>([]);

  const lastId = () => items.filter((m) => !m.pending).at(-1)?.id ?? 0;
  const nearBottom = () => !scroller || scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 120;

  function toBottom(smooth = false) {
    requestAnimationFrame(() => scroller?.scrollTo({ top: scroller.scrollHeight, behavior: smooth ? 'smooth' : 'auto' }));
  }

  $effect(() => {
    const want = id;
    who = null;
    items = [];
    error = '';
    more = true;
    api<{ with: Person; items: Message[] }>(`/api/chats/${want}`).then(
      (r) => {
        if (want !== id) return;
        who = r.with;
        items = r.items;
        more = r.items.length >= 50;
        toBottom();
        refreshBadges();
      },
      (e) => (error = errorText(e)),
    );
    const t = setInterval(async () => {
      if (document.visibilityState !== 'visible' || !who) return;
      try {
        const r = await api<{ items: Message[] }>(`/api/chats/${want}?after=${lastId()}`);
        if (want !== id || !r.items.length) return;
        const stick = nearBottom();
        const known = new Set(items.map((m) => m.id));
        items = [...items, ...r.items.filter((m) => !known.has(m.id))];
        if (stick) toBottom(true);
        refreshBadges();
      } catch {
        /* следующая попытка через 3 с */
      }
    }, 3000);
    return () => clearInterval(t);
  });

  async function older() {
    const first = items[0]?.id;
    if (!first) return;
    const h = scroller?.scrollHeight ?? 0;
    const r = await api<{ items: Message[] }>(`/api/chats/${id}?before=${first}`);
    items = [...r.items, ...items];
    more = r.items.length >= 50;
    // Держим место чтения: старые сверху не сдвигают текущее.
    requestAnimationFrame(() => scroller && (scroller.scrollTop += scroller.scrollHeight - h));
  }

  async function send(body: string, titleId: number | null = null, title: Card | null = null) {
    if (!body.trim() && !titleId) return;
    const temp = { id: -Date.now(), mine: true, body: body.trim(), createdAt: Date.now(), read: false, pending: true,
      title: title ? { id: title.id, kind: title.kind, title: title.title, year: title.year, poster: title.poster } : null };
    items = [...items, temp];
    toBottom(true);
    try {
      const m = await post<Message>(`/api/chats/${id}`, { body, titleId });
      items = items.map((x) => (x.id === temp.id ? m : x));
    } catch (e) {
      items = items.filter((x) => x.id !== temp.id);
      error = errorText(e);
    }
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    const b = text;
    text = '';
    error = '';
    send(b);
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      (e.currentTarget as HTMLTextAreaElement).form?.requestSubmit();
    }
  }

  $effect(() => {
    const query = q.trim();
    if (!picking || !query) {
      found = [];
      return;
    }
    const t = setTimeout(() => api<Card[]>(`/api/titles?q=${encodeURIComponent(query)}`).then((r) => (found = r.slice(0, 12)), () => {}), 200);
    return () => clearTimeout(t);
  });

  function share(c: Card) {
    picking = false;
    q = '';
    send(text, c.id, c);
    text = '';
  }

  const time = (ms: number) => new Date(ms).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
  const dayOf = (ms: number) => new Date(ms).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long' });
</script>

<div class="page chat-page">
  <aside class="side"><ChatList current={id} /></aside>
  <section class="chat" aria-label="Переписка">
    <header>
      <a class="btn ghost sm icon back" href="/chats" aria-label="Все чаты"><Icon name="back" size={18} /></a>
      {#if who}
        <a class="who" href="/people/{who.id}"><Avatar avatar={who.avatar} photo={who.photo} size={38} /><b>{who.name}</b></a>
      {/if}
    </header>

    <div class="msgs" bind:this={scroller}>
      {#if more && items.length}
        <button class="btn ghost sm older" onclick={older}>Ранее</button>
      {/if}
      {#each items as m, i (m.id)}
        {#if i === 0 || dayOf(items[i - 1].createdAt) !== dayOf(m.createdAt)}
          <p class="day">{dayOf(m.createdAt)}</p>
        {/if}
        <div class="msg" class:mine={m.mine} class:pending={m.pending}>
          {#if m.title}
            <a class="film" href="/title/{m.title.id}">
              <span class="pic" style:background={m.title.poster ? undefined : placeholder(m.title.title)}>
                {#if m.title.poster}<img src={m.title.poster} alt="" loading="lazy" width="44" height="66" />{/if}
              </span>
              <span><b>{m.title.title}</b><small>{m.title.year ?? ''} · Обсудим?</small></span>
            </a>
          {/if}
          {#if m.body}<p>{m.body}</p>{/if}
          <small class="t">{time(m.createdAt)}{#if m.mine}{m.pending ? ' · …' : m.read ? ' · прочитано' : ''}{/if}</small>
        </div>
      {:else}
        {#if who}<p class="hello muted">Напишите {who.name} первым — или пришлите фильм кнопкой со скрепкой.</p>{/if}
      {/each}
    </div>

    {#if error}<p class="error pad">{error}</p>{/if}
    <form class="compose" onsubmit={submit}>
      <button type="button" class="btn ghost icon" aria-label="Приложить фильм" title="Приложить фильм" onclick={() => (picking = true)}>
        <Icon name="film" size={19} />
      </button>
      <textarea class="input" rows="1" bind:value={text} maxlength="2000" placeholder="Сообщение" onkeydown={key} aria-label="Сообщение"></textarea>
      <button class="btn primary icon" aria-label="Отправить" disabled={!text.trim()}><Icon name="right" size={19} /></button>
    </form>
  </section>
</div>

<Modal bind:open={picking} title="Какой фильм обсудим?">
  <div class="pick">
    <input class="input" bind:value={q} placeholder="Название" aria-label="Название фильма" />
    <ul>
      {#each found as c (c.id)}
        <li>
          <button onclick={() => share(c)}>
            <span class="pic" style:background={c.poster ? undefined : placeholder(c.title)}>
              {#if c.poster}<img src={c.poster} alt="" width="40" height="60" />{/if}
            </span>
            <span><b>{c.title}</b><small class="muted">{c.year ?? ''}</small></span>
          </button>
        </li>
      {/each}
    </ul>
  </div>
</Modal>

<style>
  .chat-page {
    display: grid;
    grid-template-columns: 320px minmax(0, 1fr);
    gap: 18px;
    max-width: 1100px;
    margin: 0 auto;
    height: 100dvh;
    padding-bottom: calc(16px + var(--tabbar));
  }
  .side {
    overflow-y: auto;
  }
  .chat {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-radius: 20px;
    background: #141416;
    border: 1px solid var(--line);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--line);
    min-height: 60px;
  }
  .back {
    display: none;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .who b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .msgs {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px;
    overscroll-behavior: contain;
  }
  .older {
    align-self: center;
  }
  .day {
    align-self: center;
    margin: 10px 0 4px;
    font-size: 12px;
    color: var(--faint);
  }
  .msg {
    align-self: flex-start;
    display: grid;
    gap: 4px;
    max-width: min(78%, 520px);
    padding: 9px 13px 7px;
    border-radius: 18px 18px 18px 6px;
    background: #2a2a2d;
    animation: bubble 0.25s cubic-bezier(0.2, 0.9, 0.3, 1.2) both;
    transform-origin: left bottom;
  }
  .msg.mine {
    align-self: flex-end;
    border-radius: 18px 18px 6px 18px;
    background: #fff;
    color: #0a0a0a;
    transform-origin: right bottom;
  }
  .msg.pending {
    opacity: 0.7;
  }
  .msg p {
    margin: 0;
    white-space: pre-line;
    overflow-wrap: anywhere;
    font-size: 14.5px;
    line-height: 1.4;
  }
  .t {
    justify-self: end;
    font-size: 10.5px;
    opacity: 0.6;
  }
  @keyframes bubble {
    from {
      opacity: 0;
      transform: scale(0.92) translateY(6px);
    }
  }
  .film {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px;
    border-radius: 12px;
    background: rgba(127, 127, 127, 0.16);
    min-width: 0;
  }
  .film > span:last-child {
    display: grid;
    min-width: 0;
  }
  .film b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .film small {
    font-size: 11.5px;
    opacity: 0.7;
  }
  .pic {
    flex: none;
    width: 44px;
    aspect-ratio: 2 / 3;
    border-radius: 6px;
    overflow: hidden;
  }
  .pic img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .hello {
    margin: auto;
    text-align: center;
    max-width: 320px;
  }
  .compose {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 10px 12px;
    border-top: 1px solid var(--line);
  }
  .compose textarea {
    flex: 1;
    min-height: 42px;
    max-height: 140px;
    height: 42px;
    padding: 10px 14px;
    border-radius: 21px;
    resize: none;
    field-sizing: content;
  }
  .pad {
    padding: 0 16px;
  }
  .pick {
    display: grid;
    gap: 12px;
  }
  .pick ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .pick button {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px;
    border: 0;
    border-radius: 10px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .pick button:hover {
    background: var(--surface-2);
  }
  .pick button .pic {
    width: 40px;
  }
  .pick button > span:last-child {
    display: grid;
  }
  @media (max-width: 860px) {
    .chat-page {
      grid-template-columns: minmax(0, 1fr);
      padding-left: 0;
      padding-right: 0;
      padding-top: var(--header);
    }
    .side {
      display: none;
    }
    .chat {
      border-radius: 0;
      border: 0;
    }
    .back {
      display: inline-flex;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .msg {
      animation: none;
    }
  }
</style>
