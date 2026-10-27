<script lang="ts">
  import { page } from '$app/state';
  import { api, errorText, post } from '$lib/api';
  import { refreshBadges } from '$lib/social/badges.svelte';
  import ChatList from '$lib/social/ChatList.svelte';
  import type { Card, ExtCard, Message, Person } from '$lib/types';
  import { placeholder } from '$lib/ui/art';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Modal from '$lib/ui/Modal.svelte';

  // Переписка с другом: новые сообщения - опросом раз в 3 с (пока вкладка видна), свои -
  // сразу на экране. Можно приложить фильм - придёт карточкой.
  const id = $derived(Number(page.params.id));
  let who = $state<Person | null>(null);
  let items = $state<(Message & { pending?: boolean })[]>([]);
  let error = $state('');
  let text = $state('');
  let more = $state(true);
  let scroller = $state<HTMLDivElement>();
  let picking = $state(false);
  let q = $state('');
  // Фильм для сообщения: свой (titleId) или из ленты IMDb (extId).
  type Pick = { key: string; titleId: number | null; extId: string | null; title: string; year: number | null; poster: string | null; rating: number | null; kind: Card['kind'] };
  let found = $state<Pick[]>([]);
  let searching = $state(false);
  let shelves = $state<{ name: string; items: Pick[] }[] | null>(null);
  const fromCard = (c: Card): Pick => ({ key: 't' + c.id, titleId: c.id, extId: null, title: c.title, year: c.year, poster: c.poster, rating: c.rating, kind: c.kind });
  const fromExt = (e: ExtCard): Pick => {
    const base = { title: e.title, year: e.year, poster: e.poster, rating: e.rating, kind: e.kind };
    return e.local ? { ...base, key: 't' + e.local, titleId: e.local, extId: null } : { ...base, key: 'e' + e.id, titleId: null, extId: e.id };
  };
  const uniq = (list: Pick[]) => list.filter((x, i) => list.findIndex((y) => y.key === x.key) === i);

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

  async function send(body: string, film: Pick | null = null) {
    if (!body.trim() && !film) return;
    const mini = film && { title: film.title, kind: film.kind, year: film.year, poster: film.poster, rating: film.rating };
    const temp = { id: -Date.now(), mine: true, body: body.trim(), createdAt: Date.now(), read: false, pending: true,
      title: film?.titleId && mini ? { ...mini, id: film.titleId } : null,
      ext: film?.extId && mini ? { ...mini, id: film.extId } : null };
    items = [...items, temp];
    toBottom(true);
    try {
      const m = await post<Message>(`/api/chats/${id}`, { body, titleId: film?.titleId ?? null, extId: film?.extId ?? null });
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

  // Окно открылось - сразу то, что человек смотрит и хочет посмотреть.
  type Mark = { card?: Card; ext?: ExtCard };
  $effect(() => {
    if (!picking || shelves) return;
    api<Record<'watching' | 'want' | 'watched', Mark[]>>('/api/me/marks').then((m) => {
      const pick = (l: Mark[] = []) => uniq(l.map((x) => (x.card ? fromCard(x.card) : fromExt(x.ext!)))).slice(0, 12);
      shelves = [
        { name: 'Смотрю', items: pick(m.watching) },
        { name: 'Буду смотреть', items: pick(m.want) },
        { name: 'Просмотрено', items: pick(m.watched) },
      ].filter((s) => s.items.length);
    }, () => (shelves = []));
  });

  // Поиск: своя коллекция и весь каталог IMDb (там же - опечатки и русские названия).
  $effect(() => {
    const query = q.trim();
    if (!picking || query.length < 2) {
      found = [];
      searching = false;
      return;
    }
    searching = true;
    const controller = new AbortController();
    const t = setTimeout(async () => {
      const [own, world] = await Promise.all([
        api<Card[]>(`/api/titles?q=${encodeURIComponent(query)}`, { signal: controller.signal }).catch(() => [] as Card[]),
        api<ExtCard[]>(`/api/discover/search?q=${encodeURIComponent(query)}`, { signal: controller.signal }).catch(() => [] as ExtCard[]),
      ]);
      if (controller.signal.aborted) return;
      found = uniq([...own.map(fromCard), ...world.map(fromExt)]).slice(0, 24);
      searching = false;
    }, 250);
    return () => {
      clearTimeout(t);
      controller.abort();
    };
  });

  function share(film: Pick) {
    picking = false;
    q = '';
    send(text, film);
    text = '';
  }

  const kindName = (k: string) => (k === 'series' ? 'Сериал' : 'Фильм');

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
        {@const film = m.title ?? m.ext}
        {#if i === 0 || dayOf(items[i - 1].createdAt) !== dayOf(m.createdAt)}
          <p class="day">{dayOf(m.createdAt)}</p>
        {/if}
        <div class="msg" class:mine={m.mine} class:pending={m.pending}>
          {#if film}
            <a class="film" href={m.title ? `/title/${m.title.id}` : `/ext/${m.ext?.id}`}>
              <span class="pic" style:background={film.poster ? undefined : placeholder(film.title)}>
                {#if film.poster}<img src={film.poster} alt="" loading="lazy" width="64" height="96" />{/if}
              </span>
              <span class="about">
                <b>{film.title}</b>
                <small>{[kindName(film.kind), film.year].filter(Boolean).join(' - ')}{#if film.rating}<span class="star"> ★ {film.rating.toFixed(1)}</span>{/if}</small>
                <span class="open">Открыть</span>
              </span>
            </a>
          {/if}
          {#if m.body}
            {@const room = m.body.match(/\/room\/([a-z0-9]{6,12})/)}
            <p class="selectable">{room ? m.body.replace(room[0], '').replace(/:\s*$/, '') : m.body}</p>
            {#if room}<a class="btn sm white join" href="/room/{room[1]}"><Icon name="party" size={14} /> Войти в комнату</a>{/if}
          {/if}
          <small class="t">{time(m.createdAt)}{#if m.mine}{m.pending ? ' · …' : m.read ? ' · прочитано' : ''}{/if}</small>
        </div>
      {:else}
        {#if who}<p class="hello muted">Напишите {who.name} первым - или пришлите фильм кнопкой со скрепкой.</p>{/if}
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

{#snippet grid(list: Pick[])}
  <ul class="grid">
    {#each list as f (f.key)}
      <li>
        <button onclick={() => share(f)} aria-label="Прислать: {f.title}">
          <span class="pic" style:background={f.poster ? undefined : placeholder(f.title)}>
            {#if f.poster}<img src={f.poster} alt="" loading="lazy" width="96" height="144" />{/if}
          </span>
          <b>{f.title}</b>
          <small class="muted">{f.year ?? ''}{#if f.rating}<span class="star"> ★ {f.rating.toFixed(1)}</span>{/if}</small>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

<Modal bind:open={picking} title="Какой фильм обсудим?">
  <div class="pick">
    <input class="input" bind:value={q} placeholder="Название фильма или сериала" aria-label="Название фильма" />
    <div class="results">
      {#if q.trim().length >= 2}
        {#if found.length}
          {@render grid(found)}
        {:else}
          <p class="muted note">{searching ? 'Ищем…' : 'Ничего не нашлось - попробуйте иначе'}</p>
        {/if}
      {:else if shelves === null}
        <p class="muted note">Загружаем…</p>
      {:else if shelves.length}
        {#each shelves as s (s.name)}
          <h3>{s.name}</h3>
          {@render grid(s.items)}
        {/each}
      {:else}
        <p class="muted note">Введите название - найдём в коллекции и на IMDb</p>
      {/if}
    </div>
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
    gap: 12px;
    padding: 6px 14px 6px 6px;
    border-radius: 14px;
    background: rgba(127, 127, 127, 0.16);
    min-width: 0;
    width: min(300px, 100%);
  }
  .film .pic {
    width: 64px;
    border-radius: 9px;
  }
  .about {
    display: grid;
    gap: 3px;
    min-width: 0;
  }
  .film b {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    font-size: 15px;
    line-height: 1.25;
  }
  .film small {
    font-size: 12px;
    opacity: 0.75;
  }
  .star {
    color: #e8b931;
  }
  .open {
    justify-self: start;
    margin-top: 4px;
    padding: 4px 12px;
    border-radius: 99px;
    font-size: 12px;
    font-weight: 600;
    background: #fff;
    color: #0a0a0a;
  }
  .msg.mine .open {
    background: #0a0a0a;
    color: #fff;
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
  .results {
    max-height: min(60dvh, 560px);
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .results h3 {
    margin: 14px 0 8px;
    font-size: 14px;
    color: var(--muted);
  }
  .results h3:first-child {
    margin-top: 2px;
  }
  .note {
    margin: 18px 0;
    text-align: center;
  }
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(92px, 1fr));
    gap: 12px 10px;
  }
  .grid button {
    display: grid;
    gap: 4px;
    width: 100%;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .grid .pic {
    width: 100%;
    border-radius: 10px;
    transition: transform 0.2s;
  }
  .grid button:hover .pic {
    transform: scale(1.03);
  }
  .grid b {
    font-size: 13px;
    line-height: 1.25;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grid small {
    font-size: 12px;
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
  :global(.no-motion) .msg {
    animation: none;
  }
  .join {
    margin-top: 6px;
  }
</style>
