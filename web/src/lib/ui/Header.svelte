<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { post } from '$lib/api';
  import { loadMe, session } from '$lib/session.svelte';
  import { TABS, tabOf } from '$lib/library';
  import { badges, watchBadges } from '$lib/social/badges.svelte';
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  let scrolled = $state(false);
  let menu = $state(false);

  const path = $derived(page.url.pathname);
  const tab = $derived(path === '/' ? tabOf(page.url.searchParams) : null);
  const kids = $derived(!!session.me?.user.profile.kids);

  $effect(() => {
    if (kids) return;
    return watchBadges();
  });

  $effect(() => {
    const on = () => (scrolled = window.scrollY > 8);
    on();
    window.addEventListener('scroll', on, { passive: true });
    return () => window.removeEventListener('scroll', on);
  });

  $effect(() => {
    if (!menu) return;
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && (menu = false);
    addEventListener('keydown', esc);
    return () => removeEventListener('keydown', esc);
  });

  async function logout() {
    await post('/api/auth/logout').catch(() => {});
    session.me = null;
    goto('/login');
  }

  const profile = $derived(session.me?.user.profile);
  const others = $derived((session.me?.profiles ?? []).filter((p) => p.id !== profile?.id));

  async function switchTo(id: number) {
    menu = false;
    await post(`/api/profiles/${id}/select`).catch(() => {});
    await loadMe();
    // Всё на странице — для другого профиля: проще и надёжнее перезагрузить данные.
    location.assign('/');
  }
</script>

<header class:scrolled>
  <a href="/" class="brand" aria-label="На главную"><Logo /></a>

  <nav class="pill" aria-label="Разделы">
    {#each TABS as t (t.id)}
      <a href={t.href} class:on={tab === t.id} aria-current={tab === t.id ? 'page' : undefined}>{t.label}</a>
    {/each}
    {#if !kids}
      <a href="/people" class:on={path.startsWith('/people')}>Люди{#if badges.requests}<i class="dot" aria-label="есть просьбы дружбы"></i>{/if}</a>
      <a href="/chats" class:on={path.startsWith('/chats')}>Чаты{#if badges.unread}<i class="num">{badges.unread}</i>{/if}</a>
    {/if}
  </nav>

  <div class="right">
    <a class="round" href="/search" aria-label="Поиск" title="Поиск"><Icon name="search" size={21} /></a>
    {#if session.me?.manage}
      <a class="round" href="/admin?tab=folders" aria-label="Добавить фильмы" title="Добавить фильмы"><Icon name="plus" size={22} /></a>
    {/if}
    <div class="me">
      <button class="avatar" aria-label="Профиль" aria-expanded={menu} onclick={() => (menu = !menu)}>
        <Avatar avatar={profile?.avatar ?? 0} kids={profile?.kids} photo={profile?.photo} size={34} />
      </button>
      {#if menu}
        <button class="scrim" aria-label="Закрыть меню" onclick={() => (menu = false)}></button>
        <div class="menu" role="menu">
          <div class="who">
            <b>{profile?.name}{profile?.kids ? ' · детский' : ''}</b>
            <span class="muted">{session.me?.user.name} · @{session.me?.user.login}</span>
          </div>
          {#each others as p (p.id)}
            <button role="menuitem" class="prof" onclick={() => switchTo(p.id)}>
              <Avatar avatar={p.avatar} kids={p.kids} photo={p.photo} size={26} /> {p.name}
            </button>
          {/each}
          {#if !kids && profile}
            <a href="/people/{profile.id}" role="menuitem" onclick={() => (menu = false)}><Icon name="user" size={18} /> Моя страница</a>
          {/if}
          <a href="/profiles?manage=1" role="menuitem" onclick={() => (menu = false)}><Icon name="users" size={18} /> Профили и фото</a>
          <a href="/profile" role="menuitem" onclick={() => (menu = false)}><Icon name="user" size={18} /> Мой аккаунт</a>
          {#if session.me?.manage}
            <a href="/admin" role="menuitem" onclick={() => (menu = false)}><Icon name="sliders" size={18} /> Управление</a>
          {/if}
          {#if !session.me?.window}
            <button role="menuitem" onclick={logout}><Icon name="logout" size={18} /> Выйти</button>
          {/if}
        </div>
      {/if}
    </div>
  </div>
</header>

<!-- Телефон: главные разделы внизу, под большим пальцем (фильмы/сериалы/новое — капсулой на главной). -->
<nav class="tabs" class:five={!kids} aria-label="Разделы">
  <a href="/" class:on={path === '/'}><Icon name="home" size={22} /><span>Главная</span></a>
  <a href="/search" class:on={path === '/search'}><Icon name="search" size={22} /><span>Поиск</span></a>
  {#if !kids}
    <a href="/people" class:on={path.startsWith('/people')}>
      <span class="ic"><Icon name="users" size={22} />{#if badges.requests}<i class="dot"></i>{/if}</span><span>Люди</span>
    </a>
    <a href="/chats" class:on={path.startsWith('/chats')}>
      <span class="ic"><Icon name="mail" size={22} />{#if badges.unread}<i class="num">{badges.unread}</i>{/if}</span><span>Чаты</span>
    </a>
  {/if}
  <a href="/profile" class:on={path === '/profile'}>
    <Avatar avatar={profile?.avatar ?? 0} kids={profile?.kids} photo={profile?.photo} size={24} /><span>Профиль</span>
  </a>
</nav>

<style>
  header {
    position: fixed;
    inset: 0 0 auto;
    z-index: 50;
    height: var(--header);
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 12px;
    padding: env(safe-area-inset-top) var(--gutter) 0;
    box-sizing: content-box;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: saturate(160%) blur(18px);
    -webkit-backdrop-filter: saturate(160%) blur(18px);
    border-bottom: 1px solid transparent;
    transition: border-color 0.2s;
    /* При смене страницы шапка не мигает вместе со страницей. */
    view-transition-name: site-header;
  }
  header.scrolled {
    border-bottom-color: var(--line);
  }
  .brand {
    justify-self: start;
    min-width: 0;
  }

  /* Капсула разделов, как на макете: тёмная, выбранный — светлее. */
  .pill {
    display: flex;
    gap: 2px;
    padding: 4px;
    border-radius: 999px;
    background: #1c1c1e;
    border: 1px solid var(--line);
  }
  .pill a {
    padding: 6px 16px;
    border-radius: 999px;
    color: var(--muted);
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    transition:
      background 0.15s,
      color 0.15s;
  }
  .pill a:hover {
    color: var(--text);
  }
  .pill a.on {
    background: #3a3a3c;
    color: var(--text);
  }
  .pill a {
    position: relative;
  }
  .dot {
    position: absolute;
    top: 5px;
    right: 7px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #ff5c7a;
    animation: ping 1.6s ease-out infinite;
  }
  .num {
    position: absolute;
    top: -3px;
    right: -2px;
    min-width: 17px;
    height: 17px;
    padding: 0 4px;
    border-radius: 99px;
    background: #ff5c7a;
    color: #fff;
    font-size: 10.5px;
    font-style: normal;
    font-weight: 800;
    line-height: 17px;
    text-align: center;
  }
  @keyframes ping {
    70%,
    100% {
      box-shadow: 0 0 0 6px rgba(255, 92, 122, 0);
    }
    0% {
      box-shadow: 0 0 0 0 rgba(255, 92, 122, 0.6);
    }
  }

  .right {
    justify-self: end;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .round {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    color: var(--text);
    transition: background 0.15s;
  }
  .round:hover {
    background: var(--surface-2);
  }
  .me {
    position: relative;
    margin-left: 4px;
  }
  .avatar {
    display: block;
    padding: 0;
    border: 0;
    border-radius: 50%;
    overflow: hidden;
    background: none;
    cursor: pointer;
    transition: transform 0.15s;
  }
  .avatar:hover {
    transform: scale(1.06);
  }
  .scrim {
    position: fixed;
    inset: 0;
    background: transparent;
    border: 0;
    cursor: default;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 46px;
    width: max-content;
    min-width: 220px;
    max-width: calc(100vw - 32px);
    padding: 6px;
    border-radius: var(--radius);
    background: rgba(28, 28, 30, 0.97);
    backdrop-filter: blur(16px);
    border: 1px solid var(--line);
    box-shadow: 0 20px 50px -10px rgba(0, 0, 0, 0.8);
    display: grid;
  }
  .who {
    display: grid;
    padding: 8px 12px 10px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 4px;
    font-size: 14px;
    overflow-wrap: anywhere;
  }
  .who span {
    font-size: 12px;
  }
  .menu a,
  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border-radius: 8px;
    border: 0;
    background: none;
    text-align: left;
    cursor: pointer;
    font-size: 14px;
  }
  .menu a:hover,
  .menu button:hover {
    background: var(--surface-2);
  }
  .menu .prof {
    font-weight: 600;
  }

  .tabs {
    display: none;
  }

  /* Ниже 1000 px капсула не помещается — разделы переезжают в нижнюю панель (одна из двух
     навигаций видна всегда: e2e проверяет). */
  @media (max-width: 1000px) {
    header {
      grid-template-columns: 1fr auto;
    }
    .pill {
      display: none;
    }
    .right .round[href='/search'] {
      display: none;
    }
    .tabs {
      position: fixed;
      inset: auto 0 0;
      z-index: 50;
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      padding: 6px 4px calc(6px + env(safe-area-inset-bottom));
      background: rgba(0, 0, 0, 0.88);
      backdrop-filter: saturate(160%) blur(18px);
      -webkit-backdrop-filter: saturate(160%) blur(18px);
      border-top: 1px solid var(--line);
      view-transition-name: site-tabs;
    }
    .tabs.five {
      grid-template-columns: repeat(5, 1fr);
    }
    .tabs a {
      display: grid;
      justify-items: center;
      gap: 2px;
      padding: 4px 0;
      font-size: 11px;
      font-weight: 600;
      color: var(--faint);
      transition: color 0.15s;
    }
    .tabs a.on {
      color: var(--text);
    }
    .tabs .ic {
      position: relative;
      display: grid;
    }
    .tabs .dot {
      top: -1px;
      right: -3px;
    }
    .tabs .num {
      top: -6px;
      right: -10px;
    }
  }
  @media (max-width: 640px) {
    .brand :global(.word) {
      display: none;
    }
  }
</style>
