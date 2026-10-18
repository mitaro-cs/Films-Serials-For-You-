<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { post } from '$lib/api';
  import { session } from '$lib/session.svelte';
  import { TABS, tabOf } from '$lib/library';
  import { badges, watchBadges } from '$lib/social/badges.svelte';
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  // Шапка как у Apple TV: стеклянная капсула разделов по центру (поиск — в ней же),
  // слева знак, справа «добавить» и аватар. Поверх героя главной — прозрачная.
  let scrolled = $state(false);
  let menu = $state(false);

  const path = $derived(page.url.pathname);
  const tab = $derived(path === '/' ? tabOf(page.url.searchParams) : null);
  const profile = $derived(session.me?.user.profile);

  $effect(() => {
    if (session.me) return watchBadges();
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
</script>

<header class:scrolled>
  <a href="/" class="brand" aria-label="На главную"><Logo /></a>

  <nav class="pill" aria-label="Разделы">
    {#each TABS as t (t.id)}
      <a href={t.href} class:on={tab === t.id} aria-current={tab === t.id ? 'page' : undefined}>{t.label}</a>
    {/each}
    <a href="/discover" class:on={path.startsWith('/discover')}>Обзор</a>
    <a href="/lists" class:on={path.startsWith('/lists')}>Списки</a>
    <a href="/people" class:on={path.startsWith('/people')}>Люди{#if badges.requests}<i class="dot" aria-label="есть просьбы дружбы"></i>{/if}</a>
    <a href="/chats" class:on={path.startsWith('/chats')}>Чаты{#if badges.unread}<i class="num">{badges.unread}</i>{/if}</a>
    <a href="/search" class="find" class:on={path === '/search'} aria-label="Поиск" title="Поиск"><Icon name="search" size={18} /></a>
  </nav>

  <div class="right">
    <a class="round find-m" href="/search" aria-label="Поиск" title="Поиск"><Icon name="search" size={21} /></a>
    {#if session.me?.manage}
      {#if badges.wishes}
        <a class="round" href="/admin?tab=wishes" aria-label="Просьбы зрителей: {badges.wishes}" title="Просьбы зрителей">
          <Icon name="flag" size={20} /><i class="num">{badges.wishes}</i>
        </a>
      {/if}
      <a class="round" href="/admin?tab=folders" aria-label="Добавить фильмы" title="Добавить фильмы"><Icon name="plus" size={22} /></a>
    {/if}
    <div class="me">
      <button class="avatar" aria-label="Аккаунт" aria-expanded={menu} onclick={() => (menu = !menu)}>
        <Avatar avatar={profile?.avatar ?? 0} photo={profile?.photo} size={34} />
      </button>
      {#if menu}
        <button class="scrim" aria-label="Закрыть меню" onclick={() => (menu = false)}></button>
        <div class="menu" role="menu">
          <div class="who">
            <b>{session.me?.user.name}</b>
            <span class="muted">@{session.me?.user.login}</span>
          </div>
          {#if profile}
            <a href="/people/{profile.id}" role="menuitem" onclick={() => (menu = false)}><Icon name="user" size={18} /> Моя страница</a>
          {/if}
          <a href="/profile" role="menuitem" onclick={() => (menu = false)}><Icon name="image" size={18} /> Профиль и оформление</a>
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

<!-- Телефон и планшет: главные разделы внизу, под большим пальцем. -->
<nav class="tabs" aria-label="Разделы">
  <a href="/" class:on={path === '/'}><Icon name="home" size={22} /><span>Главная</span></a>
  <a href="/discover" class:on={path.startsWith('/discover')}><Icon name="globe" size={22} /><span>Обзор</span></a>
  <a href="/lists" class:on={path.startsWith('/lists')}><Icon name="bookmark" size={22} /><span>Списки</span></a>
  <a href="/people" class:on={path.startsWith('/people')}>
    <span class="ic"><Icon name="users" size={22} />{#if badges.requests}<i class="dot"></i>{/if}</span><span>Люди</span>
  </a>
  <a href="/chats" class:on={path.startsWith('/chats')}>
    <span class="ic"><Icon name="mail" size={22} />{#if badges.unread}<i class="num">{badges.unread}</i>{/if}</span><span>Чаты</span>
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
    /* Над героем — только мягкая тень сверху, чтобы капсула читалась на светлом кадре. */
    background: linear-gradient(180deg, rgba(0, 0, 0, 0.55), transparent);
    view-transition-name: site-header;
  }
  header::before {
    content: '';
    position: absolute;
    inset: 0;
    z-index: -1;
    background: rgba(8, 8, 10, 0.94);
    border-bottom: 1px solid var(--line);
    opacity: 0;
    transition: opacity 0.25s;
  }
  header.scrolled::before {
    opacity: 1;
  }
  .brand {
    justify-self: start;
    min-width: 0;
  }

  /* Капсула разделов: тёмное стекло, выбранный — светлая «таблетка», как у Apple TV. */
  .pill {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 5px;
    border-radius: 999px;
    background: rgba(40, 37, 35, 0.9);
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.08),
      0 8px 30px -10px rgba(0, 0, 0, 0.6);
  }
  .pill a {
    position: relative;
    display: grid;
    place-items: center;
    height: 36px;
    padding: 0 18px;
    border-radius: 999px;
    color: rgba(255, 255, 255, 0.82);
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    transition:
      background 0.2s,
      color 0.2s,
      transform 0.2s;
  }
  .pill a:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.08);
  }
  .pill a.on {
    background: rgba(235, 235, 240, 0.92);
    color: #111;
  }
  .pill .find {
    width: 36px;
    padding: 0;
  }
  .dot {
    position: absolute;
    top: 6px;
    right: 8px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #ff5c7a;
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

  .right {
    justify-self: end;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .round {
    position: relative;
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
    border: 1px solid var(--line);
    box-shadow: 0 20px 50px -10px rgba(0, 0, 0, 0.8);
    display: grid;
    transform-origin: top right;
    animation: pop 0.18s ease-out;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-4px);
    }
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

  .tabs {
    display: none;
  }
  .find-m {
    display: none;
  }

  /* Ниже 1000 px капсула не помещается — разделы в нижней панели (Фильмы и Сериалы —
     капсулой на главной). Одна из двух навигаций видна всегда: e2e проверяет. */
  @media (max-width: 1000px) {
    header {
      grid-template-columns: 1fr auto;
    }
    .pill {
      display: none;
    }
    .find-m {
      display: grid;
    }
    .tabs {
      position: fixed;
      inset: auto 0 0;
      z-index: 50;
      display: grid;
      grid-template-columns: repeat(5, 1fr);
      padding: 6px 4px calc(6px + env(safe-area-inset-bottom));
      background: rgba(8, 8, 10, 0.96);
      border-top: 1px solid var(--line);
      view-transition-name: site-tabs;
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
  /* Капсула из восьми разделов: на узком компьютере слово рядом со знаком не помещается. */
  @media (max-width: 1240px) {
    .brand :global(.word) {
      display: none;
    }
  }
  @media (max-width: 1000px) {
    .brand :global(.word) {
      display: inline;
    }
  }
  @media (max-width: 640px) {
    .brand :global(.word) {
      display: none;
    }
  }
</style>
