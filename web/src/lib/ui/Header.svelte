<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { post } from '$lib/api';
  import { loadMe, session } from '$lib/session.svelte';
  import { TABS, tabOf } from '$lib/library';
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  let scrolled = $state(false);
  let menu = $state(false);

  const path = $derived(page.url.pathname);
  const tab = $derived(path === '/' ? tabOf(page.url.searchParams) : null);

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
  </nav>

  <div class="right">
    <a class="round" href="/search" aria-label="Поиск" title="Поиск"><Icon name="search" size={21} /></a>
    {#if session.me?.manage}
      <a class="round" href="/admin?tab=folders" aria-label="Добавить фильмы" title="Добавить фильмы"><Icon name="plus" size={22} /></a>
    {/if}
    <div class="me">
      <button class="avatar" aria-label="Профиль" aria-expanded={menu} onclick={() => (menu = !menu)}>
        <Avatar avatar={profile?.avatar ?? 0} kids={profile?.kids} size={34} />
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
              <Avatar avatar={p.avatar} kids={p.kids} size={26} /> {p.name}
            </button>
          {/each}
          <a href="/profiles?manage=1" role="menuitem" onclick={() => (menu = false)}><Icon name="users" size={18} /> Профили</a>
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

<!-- Телефон: те же разделы внизу, под большим пальцем. -->
<nav class="tabs" aria-label="Разделы">
  {#each TABS as t (t.id)}
    <a href={t.href} class:on={tab === t.id}><Icon name={t.icon} size={22} /><span>{t.label}</span></a>
  {/each}
  <a href="/search" class:on={path === '/search'}><Icon name="search" size={22} /><span>Поиск</span></a>
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

  @media (max-width: 760px) {
    header {
      grid-template-columns: 1fr auto;
    }
    .pill {
      display: none;
    }
  }
  @media (max-width: 640px) {
    .brand :global(.word) {
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
      grid-template-columns: repeat(4, 1fr);
      padding: 6px 4px calc(6px + env(safe-area-inset-bottom));
      background: rgba(0, 0, 0, 0.88);
      backdrop-filter: saturate(160%) blur(18px);
      -webkit-backdrop-filter: saturate(160%) blur(18px);
      border-top: 1px solid var(--line);
    }
    .tabs a {
      display: grid;
      justify-items: center;
      gap: 2px;
      padding: 4px 0;
      font-size: 11px;
      font-weight: 600;
      color: var(--faint);
    }
    .tabs a.on {
      color: var(--text);
    }
  }
</style>
