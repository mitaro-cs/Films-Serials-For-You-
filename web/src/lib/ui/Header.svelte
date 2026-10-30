<script lang="ts">
  import { goto } from '$app/navigation';
  import { page, navigating } from '$app/state';
  import { post } from '$lib/api';
  import { session } from '$lib/session.svelte';
  import { TABS, tabOf } from '$lib/library';
  import { badges, watchBadges } from '$lib/social/badges.svelte';
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';
  import Logo from './Logo.svelte';

  // Шапка как у Apple TV: стеклянная капсула разделов по центру (поиск - в ней же), слева знак,
  // справа «добавить» и аватар. Люди, Чаты и Списки - в меню аватара (значок на нём - есть
  // непрочитанное или просьбы дружбы). На телефоне то же меню открывается из нижней панели.
  let scrolled = $state(false);
  let menu = $state<'' | 'top' | 'tab'>('');

  const navigationUrl = $derived(navigating.to?.url ?? page.url);
  const path = $derived(navigationUrl.pathname);
  const tab = $derived(path === '/' ? tabOf(navigationUrl.searchParams) : null);
  const profile = $derived(session.me?.user.profile);
  const alert = $derived(badges.requests + badges.unread);

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
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && (menu = '');
    addEventListener('keydown', esc);
    return () => removeEventListener('keydown', esc);
  });

  // Перешли куда-то - меню закрывается само.
  $effect(() => {
    void path;
    menu = '';
  });

  async function logout() {
    await post('/api/auth/logout').catch(() => {});
    session.me = null;
    goto('/login');
  }

  type Item = { href: string; icon: IconName; label: string; count?: number; dot?: boolean };
  const items = $derived<Item[]>([
    ...(profile ? [{ href: `/people/${profile.id}`, icon: 'user' as const, label: 'Моя страница' }] : []),
    { href: '/people', icon: 'users', label: 'Люди', dot: badges.requests > 0 },
    { href: '/chats', icon: 'mail', label: 'Чаты', count: badges.unread },
    { href: '/lists', icon: 'list', label: 'Списки' },
  ]);
  const more = $derived<Item[]>([
    { href: '/profile', icon: 'image', label: 'Профиль и оформление' },
    ...(session.me?.manage ? [{ href: '/admin', icon: 'sliders' as const, label: 'Управление' }] : []),
  ]);
  const inMenu = $derived(['/people', '/chats', '/lists', '/profile'].some((p) => path.startsWith(p)));
</script>

<header class:scrolled class:home={path === "/"} class:detail={path.startsWith("/title/") || path.startsWith("/ext/")}>
  <a href="/" class="brand" aria-label="На главную"><Logo /></a>

  <nav class="pill" aria-label="Разделы">
    {#each TABS as t (t.id)}
      <a href={t.href} class:on={tab === t.id} aria-current={tab === t.id ? 'page' : undefined}><Icon name={t.id === 'home' ? 'home' : t.id === 'movies' ? 'film' : t.id === 'series' ? 'tv' : 'sparkle'} size={15} />{t.label}</a>
    {/each}
    <a href="/discover" class:on={path.startsWith('/discover') || path.startsWith('/ext')}><Icon name="globe" size={15} />Обзор</a>
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
      <button
        class="avatar"
        aria-label={alert ? `Меню: ${alert} новых` : 'Меню'}
        aria-expanded={menu === 'top'}
        onclick={() => (menu = menu ? '' : 'top')}
      >
        <Avatar avatar={profile?.avatar ?? 0} photo={profile?.photo} size={34} />
        {#if alert}<i class="num on-ava">{alert}</i>{/if}
      </button>
    </div>
  </div>
</header>

{#if menu}
  <button class="scrim" class:dim={menu === 'tab'} aria-label="Закрыть меню" onclick={() => (menu = '')}></button>
  <div class="menu" class:sheet={menu === 'tab'} role="menu" aria-label="Меню">
    <a class="who" href={profile ? `/people/${profile.id}` : '/profile'} role="menuitem">
      <Avatar avatar={profile?.avatar ?? 0} photo={profile?.photo} size={44} />
      <span>
        <b>{session.me?.user.name}</b>
        <span class="muted">@{session.me?.user.login}</span>
      </span>
    </a>
    {#each items as it, k (it.href)}
      <a href={it.href} role="menuitem" class:on={path.startsWith(it.href)} style:--k={k}>
        <span class="ic"><Icon name={it.icon} size={19} />{#if it.dot}<i class="dot"></i>{/if}</span>
        <span class="lbl">{it.label}</span>
        {#if it.count}<i class="cnt">{it.count}</i>{/if}
      </a>
    {/each}
    <hr />
    {#each more as it, k (it.href)}
      <a href={it.href} role="menuitem" class:on={path.startsWith(it.href)} style:--k={k + items.length}>
        <span class="ic"><Icon name={it.icon} size={19} /></span><span class="lbl">{it.label}</span>
      </a>
    {/each}
    {#if !session.me?.window}
      <button role="menuitem" onclick={logout} style:--k={items.length + more.length}>
        <span class="ic"><Icon name="logout" size={19} /></span><span class="lbl">Выйти</span>
      </button>
    {/if}
  </div>
{/if}

<!-- Телефон и планшет: главные разделы внизу, под большим пальцем; «Я» - то же меню. -->
<nav class="tabs" aria-label="Разделы">
  <a href="/" class:on={path === '/' && tab === 'home'}><Icon name="home" size={22} /><span>Главная</span></a>
  <a href="/?tab=movies" class:on={tab === 'movies'}><Icon name="film" size={22} /><span>Фильмы</span></a>
  <a href="/?tab=series" class:on={tab === 'series'}><Icon name="tv" size={22} /><span>Сериалы</span></a>
  <a href="/?tab=anime" class:on={tab === 'anime'}><Icon name="star" size={22} /><span>Аниме</span></a>
  <a href="/discover" class:on={path.startsWith('/discover') || path.startsWith('/ext')}><Icon name="globe" size={22} /><span>Обзор</span></a>
  <button class:on={inMenu || menu === 'tab'} aria-expanded={menu === 'tab'} onclick={() => (menu = menu ? '' : 'tab')}>
    <span class="ic"><Avatar avatar={profile?.avatar ?? 0} photo={profile?.photo} size={24} />{#if alert}<i class="num">{alert}</i>{/if}</span>
    <span>Я</span>
  </button>
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
    /* Над героем - только мягкая тень сверху, чтобы капсула читалась на светлом кадре. */
    background: linear-gradient(180deg, rgba(0, 0, 0, 0.55), transparent);
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

  /* Капсула разделов: тёмное стекло, выбранный - светлая «таблетка», как у Apple TV. */
  .pill {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 5px;
    border-radius: 999px;
    /* Над кадром героя - в его цвет, как у Apple TV (коричневая капсула над оранжевым кадром). */
    background: color-mix(in srgb, var(--hero-tint, #2a2724) 45%, rgba(20, 20, 20, 0.84));
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.08),
      0 8px 30px -10px rgba(0, 0, 0, 0.6);
    transition: background 0.5s;
  }
  .pill a {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    flex: 0 0 auto;
    height: 36px;
    padding: 0 14px;
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
  .pill a:active {
    transform: scale(0.95);
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
    top: -2px;
    right: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #ff5c7a;
    box-shadow: 0 0 0 2px rgba(28, 28, 30, 1);
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
    animation: badge 0.5s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes badge {
    from {
      transform: scale(0);
    }
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
    position: relative;
    display: block;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: none;
    cursor: pointer;
    transition: transform 0.2s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .avatar:hover {
    transform: scale(1.08);
  }
  .avatar:active {
    transform: scale(0.94);
  }
  .on-ava {
    top: -4px;
    right: -6px;
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: transparent;
    border: 0;
    cursor: default;
  }
  .scrim.dim {
    background: rgba(0, 0, 0, 0.5);
    animation: fade 0.2s;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  /* Меню аватара: пружинит из угла, пункты - лесенкой. */
  .menu {
    position: fixed;
    z-index: 71;
    right: max(var(--gutter), env(safe-area-inset-right));
    top: calc(var(--header) + env(safe-area-inset-top) - 6px);
    width: 280px;
    max-width: calc(100vw - 32px);
    padding: 8px;
    border-radius: 18px;
    background: rgba(26, 26, 28, 0.98);
    border: 1px solid var(--line);
    box-shadow: 0 30px 70px -14px rgba(0, 0, 0, 0.85);
    display: grid;
    transform-origin: top right;
    animation: pop 0.32s cubic-bezier(0.2, 0.9, 0.3, 1.25);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.88) translateY(-8px);
    }
  }
  /* С телефона - лист снизу, над нижней панелью. */
  .menu.sheet {
    top: auto;
    left: 12px;
    right: 12px;
    bottom: calc(var(--tabbar) + 10px);
    width: auto;
    max-width: none;
    transform-origin: bottom right;
    animation: sheet 0.36s cubic-bezier(0.2, 0.9, 0.3, 1.15);
  }
  @keyframes sheet {
    from {
      opacity: 0;
      transform: translateY(24px) scale(0.97);
    }
  }
  .who {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px 12px;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--line);
    border-radius: 12px 12px 0 0;
    min-width: 0;
  }
  .who span {
    display: grid;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .who .muted {
    font-size: 12.5px;
  }
  .menu a:not(.who),
  .menu button {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 12px;
    border: 0;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    font-size: 15px;
    font-weight: 500;
    animation: item 0.35s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(var(--k, 0) * 28ms + 40ms);
    transition:
      background 0.15s,
      transform 0.15s;
  }
  @keyframes item {
    from {
      opacity: 0;
      transform: translateX(10px);
    }
  }
  .menu a:hover,
  .menu button:hover {
    background: var(--surface-2);
  }
  .menu a:active,
  .menu button:active {
    transform: scale(0.98);
  }
  .menu a.on {
    background: rgba(255, 255, 255, 0.08);
  }
  .menu .ic {
    position: relative;
    display: grid;
    color: var(--muted);
  }
  .menu .lbl {
    flex: 1;
  }
  .cnt {
    min-width: 22px;
    height: 22px;
    padding: 0 7px;
    border-radius: 99px;
    background: #ff5c7a;
    color: #fff;
    font-size: 12px;
    font-style: normal;
    font-weight: 800;
    line-height: 22px;
    text-align: center;
  }
  hr {
    border: 0;
    border-top: 1px solid var(--line);
    margin: 4px 0;
  }

  .tabs {
    display: none;
  }
  .find-m {
    display: none;
  }

  /* Ниже 1000 px капсула не помещается - разделы в нижней панели. Одна из двух навигаций
     видна всегда: e2e проверяет. */
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
      /* Свой слой: на iPhone панель не уезжает со страницей при прокрутке и смене панелей Safari. */
      transform: translate3d(0, 0, 0);
      backface-visibility: hidden;
      display: grid;
      grid-template-columns: repeat(6, minmax(0, 1fr));
      padding: 6px 4px calc(6px + env(safe-area-inset-bottom));
      background: rgba(8, 8, 10, 0.96);
      border-top: 1px solid var(--line);
    }
    .tabs a,
    .tabs button {
      display: grid;
      justify-items: center;
      gap: 2px;
      min-width: 0;
      padding: 4px 0;
      border: 0;
      background: none;
      font: inherit;
      /* Шесть разделов на 320 px: подпись уменьшается с экраном и не выходит за свою кнопку. */
      font-size: clamp(9.5px, 3vw, 11px);
      font-weight: 600;
      color: var(--faint);
      cursor: pointer;
      transition: color 0.15s;
    }
    .tabs a > span,
    .tabs button > span:last-child {
      max-width: 100%;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .tabs .on {
      color: var(--text);
    }
    /* Значок раздела пружинит при нажатии и подпрыгивает, когда раздел выбран. */
    .tabs :global(svg),
    .tabs .ic {
      transition: transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
    }
    .tabs a:active :global(svg),
    .tabs button:active .ic {
      transform: scale(0.85);
    }
    .tabs a.on :global(svg),
    .tabs button.on .ic {
      animation: tab-pop 0.4s cubic-bezier(0.2, 0.8, 0.2, 1);
    }
    @keyframes tab-pop {
      40% {
        transform: translateY(-3px) scale(1.12);
      }
    }
    .tabs .ic {
      position: relative;
      display: grid;
      height: 22px;
      place-items: center;
    }
    .tabs button:not(.on) .ic {
      opacity: 0.75;
    }
    .tabs .num {
      top: -6px;
      right: -10px;
    }
    /* На телефоне аватар в шапке не нужен - он в нижней панели. */
    .me {
      display: none;
    }
  }
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


  .pill { background: #000; border: 1px solid #ffffff20; box-shadow: none; }
  .pill a { display: flex; gap: 7px; padding-inline: 14px; }
  .pill a.on { background: #000; color: #fff; box-shadow: inset 0 0 0 1px #ffffff80; }
  .tabs { background: #000; backdrop-filter: none; }
  /* Телефон - «Кинозал»: верх отдан кадру фильма, знака Nova в шапке нет. */
  @media (max-width: 640px) {
    .brand {
      visibility: hidden;
    }
  }
  /* «Жидкое стекло», как у Apple: только капсула разделов и круглые кнопки шапки - маленькая
     площадь, видеокарту хоста не грузит (на всю страницу размытие убрали в 0.6). */
  .pill {
    background: rgba(255, 255, 255, 0.1);
    -webkit-backdrop-filter: blur(22px) saturate(180%);
    backdrop-filter: blur(22px) saturate(180%);
    border: 1px solid rgba(255, 255, 255, 0.22);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.28),
      inset 0 -1px 0 rgba(255, 255, 255, 0.06),
      0 10px 30px -12px rgba(0, 0, 0, 0.6);
  }
  .pill a.on {
    background: rgba(255, 255, 255, 0.92);
    color: #0a0a0a;
    box-shadow: none;
  }
  .pill .find:not(.on) { background:transparent; color:#fff; }
  .pill .find:not(.on):hover { background:#ffffff20; }
  .pill { max-width:100%; min-width:0; }
  .pill a { min-width:0; }
  .right .round {
    background: rgba(255, 255, 255, 0.1);
    -webkit-backdrop-filter: blur(22px) saturate(180%);
    backdrop-filter: blur(22px) saturate(180%);
    border: 1px solid rgba(255, 255, 255, 0.22);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.28),
      inset 0 -1px 0 rgba(255, 255, 255, 0.06),
      0 10px 30px -12px rgba(0, 0, 0, 0.6);
  }
  .right .round:hover {
    background: rgba(255, 255, 255, 0.18);
  }
  /* Страница фильма на телефоне - без шапки: кадр от самого верха, кнопки - на нём. */
  @media (max-width: 640px) {
    header.detail {
      display: none;
    }
  }
</style>
