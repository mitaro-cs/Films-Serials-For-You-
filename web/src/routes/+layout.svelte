<script lang="ts">
  import '../app.css';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Ask from '$lib/ui/Ask.svelte';
  import Header from '$lib/ui/Header.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Logo from '$lib/ui/Logo.svelte';
  import StatusMenu from '$lib/ui/StatusMenu.svelte';
  import { loadMe, net, session } from '$lib/session.svelte';

  let { children } = $props();

  const AUTH = ['/login', '/join', '/register'];
  const path = $derived(page.url.pathname);
  const auth = $derived(AUTH.some((p) => path.startsWith(p)));
  const bare = $derived(auth || path.startsWith('/watch') || path === '/profiles');

  $effect(() => {
    if (auth || session.me) {
      session.loading = false;
      return;
    }
    loadMe().then((me) => {
      const here = location.pathname + location.search;
      if (!me && !AUTH.some((p) => location.pathname.startsWith(p))) {
        goto(`/login?next=${encodeURIComponent(here)}`, { replaceState: true });
      } else if (me && me.profiles.length > 1 && !me.user.profileChosen && location.pathname !== '/profiles') {
        // Как у Netflix: на новом устройстве сначала «Кто смотрит?».
        goto(`/profiles?next=${encodeURIComponent(here)}`, { replaceState: true });
      }
    });
  });

  $effect(() => {
    document.title = session.me?.site.name ?? 'Films & Serials For You';
  });
</script>

<Ask />

{#if net.down}
  <div class="down" role="status">
    <Icon name="offline" size={18} /> Компьютер хоста сейчас недоступен — фильмы вернутся, когда он включится
  </div>
{/if}

{#if bare}
  {@render children()}
{:else if session.me}
  <Header />
  <main>{@render children()}</main>
  <StatusMenu />
  <footer>
    <Logo />
    <p class="muted">Фильмы и сериалы с компьютера {session.me.site.name === 'Films & Serials For You' ? 'хоста' : `«${session.me.site.name}»`}. Смотрите онлайн или скачивайте.</p>
    <span class="faint">Версия {session.me.site.version}</span>
  </footer>
{:else if session.loading}
  <div class="boot"><Logo /></div>
{/if}

<style>
  main {
    min-height: 70vh;
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 28px;
    margin-top: 64px;
    padding: 28px var(--gutter) calc(28px + env(safe-area-inset-bottom));
    border-top: 1px solid var(--line);
    background: rgba(0, 0, 0, 0.25);
    font-size: 13px;
  }
  footer p {
    margin: 0;
    flex: 1;
    min-width: 220px;
  }
  @media (max-width: 640px) {
    footer {
      padding-bottom: calc(88px + env(safe-area-inset-bottom));
    }
  }
  .boot {
    min-height: 100dvh;
    display: grid;
    place-items: center;
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.5;
    }
  }
  .down {
    position: fixed;
    z-index: 100;
    left: 50%;
    bottom: calc(84px + env(safe-area-inset-bottom));
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: calc(100vw - 32px);
    padding: 10px 16px;
    border-radius: 999px;
    background: rgba(20, 20, 22, 0.95);
    border: 1px solid var(--line);
    box-shadow: 0 16px 40px -10px rgba(0, 0, 0, 0.7);
    font-size: 13px;
  }
</style>
