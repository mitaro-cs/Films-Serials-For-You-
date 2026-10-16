<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { post } from '$lib/api';
  import { session } from '$lib/session.svelte';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  let scrolled = $state(false);
  let menu = $state(false);
  let q = $state('');

  const NAV = [
    { href: '/', label: 'Главная', icon: 'home' },
    { href: '/movies', label: 'Фильмы', icon: 'film' },
    { href: '/series', label: 'Сериалы', icon: 'tv' },
    { href: '/list', label: 'Мой список', icon: 'bookmark' },
  ] as const;

  const path = $derived(page.url.pathname);
  const home = $derived(path === '/');

  $effect(() => {
    const on = () => (scrolled = window.scrollY > 24);
    on();
    window.addEventListener('scroll', on, { passive: true });
    return () => window.removeEventListener('scroll', on);
  });

  $effect(() => {
    if (path === '/search') q = page.url.searchParams.get('q') ?? '';
  });

  function search(e: SubmitEvent) {
    e.preventDefault();
    goto(`/search?q=${encodeURIComponent(q.trim())}`, { keepFocus: true });
  }

  function typed() {
    if (path === '/search') goto(`/search?q=${encodeURIComponent(q)}`, { replaceState: true, keepFocus: true, noScroll: true });
  }

  async function logout() {
    await post('/api/auth/logout').catch(() => {});
    session.me = null;
    goto('/login');
  }

  const initials = $derived((session.me?.user.name ?? '?').trim().slice(0, 1).toUpperCase());
</script>

<header class:scrolled class:home>
  <div class="left">
    {#if !home}
      <button class="btn ghost sm icon back" aria-label="Назад" onclick={() => history.back()}><Icon name="back" /></button>
    {/if}
    <a href="/" class="brand" aria-label="На главную"><Logo /></a>
    <nav>
      {#each NAV as n (n.href)}
        <a href={n.href} class:active={path === n.href}>{n.label}</a>
      {/each}
    </nav>
  </div>
  <form class="search" role="search" onsubmit={search}>
    <Icon name="search" size={18} />
    <input type="search" placeholder="Поиск фильмов и сериалов" bind:value={q} oninput={typed} aria-label="Поиск" />
  </form>
  <div class="right">
    {#if session.me?.manage}
      <a class="btn ghost sm icon" href="/admin" aria-label="Управление" title="Управление"><Icon name="sliders" /></a>
    {/if}
    <div class="me">
      <button class="avatar" aria-label="Профиль" aria-expanded={menu} onclick={() => (menu = !menu)}>{initials}</button>
      {#if menu}
        <button class="scrim" aria-label="Закрыть меню" onclick={() => (menu = false)}></button>
        <div class="menu" role="menu">
          <div class="who">
            <b>{session.me?.user.name}</b>
            <span class="muted">@{session.me?.user.login}</span>
          </div>
          <a href="/profile" role="menuitem" onclick={() => (menu = false)}><Icon name="user" size={18} /> Профиль</a>
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

<nav class="tabs" aria-label="Разделы">
  {#each NAV as n (n.href)}
    <a href={n.href} class:active={path === n.href}><Icon name={n.icon} size={22} /><span>{n.label}</span></a>
  {/each}
  <a href="/search" class:active={path === '/search'}><Icon name="search" size={22} /><span>Поиск</span></a>
</nav>

<style>
  header {
    position: fixed;
    inset: 0 0 auto;
    z-index: 50;
    height: var(--header);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 0 var(--gutter);
    padding-top: env(safe-area-inset-top);
    box-sizing: content-box;
    transition:
      background 0.25s,
      border-color 0.25s;
    border-bottom: 1px solid transparent;
    background: linear-gradient(180deg, rgba(11, 10, 26, 0.85), rgba(11, 10, 26, 0));
  }
  header.scrolled,
  header:not(.home) {
    background: rgba(11, 10, 26, 0.78);
    backdrop-filter: saturate(160%) blur(18px);
    -webkit-backdrop-filter: saturate(160%) blur(18px);
    border-bottom-color: var(--line);
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .left,
  .right {
    flex: none;
  }
  .left nav {
    display: flex;
    gap: 4px;
    margin-left: 18px;
  }
  .left nav a {
    padding: 6px 12px;
    border-radius: 999px;
    color: var(--muted);
    font-weight: 600;
    font-size: 14px;
    white-space: nowrap;
  }
  .left nav a:hover {
    color: var(--text);
  }
  .left nav a.active {
    color: var(--text);
    background: var(--surface-2);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 0 1 360px;
    min-width: 150px;
    margin: 0 auto;
    height: 40px;
    padding: 0 14px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid var(--line);
    color: var(--muted);
    transition: border-color 0.15s;
  }
  .search:focus-within {
    border-color: var(--accent-2);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
  }
  .search input::-webkit-search-cancel-button {
    display: none;
  }
  .me {
    position: relative;
  }
  .avatar {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    border: 0;
    background: var(--accent-grad);
    color: #fff;
    font-weight: 800;
    cursor: pointer;
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
    top: 48px;
    min-width: 220px;
    padding: 6px;
    border-radius: var(--radius);
    background: rgba(24, 21, 54, 0.96);
    backdrop-filter: blur(16px);
    border: 1px solid var(--line);
    box-shadow: 0 20px 50px -10px rgba(0, 0, 0, 0.7);
    display: grid;
  }
  .who {
    display: grid;
    padding: 8px 12px 10px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 4px;
    font-size: 14px;
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

  @media (max-width: 1080px) {
    .left nav {
      display: none;
    }
  }
  @media (max-width: 640px) {
    .search,
    .brand :global(.word) {
      display: none;
    }
    .tabs {
      position: fixed;
      inset: auto 0 0;
      z-index: 50;
      display: grid;
      grid-template-columns: repeat(5, 1fr);
      margin: 0;
      padding: 6px 4px calc(6px + env(safe-area-inset-bottom));
      background: rgba(11, 10, 26, 0.88);
      backdrop-filter: saturate(160%) blur(18px);
      -webkit-backdrop-filter: saturate(160%) blur(18px);
      border-top: 1px solid var(--line);
    }
    .tabs a {
      display: grid;
      justify-items: center;
      gap: 2px;
      padding: 4px 0;
      font-size: 10px;
      font-weight: 600;
      color: var(--faint);
    }
    .tabs a.active {
      color: var(--text);
    }
  }
</style>
