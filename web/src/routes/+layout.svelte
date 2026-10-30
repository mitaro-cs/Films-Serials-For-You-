<script lang="ts">
  import '../app.css';
  import { goto } from '$app/navigation';
  import Loading from '$lib/ui/Loading.svelte';
  import { applyMotion } from '$lib/motion';
  import { page, navigating } from '$app/state';
  import Ask from '$lib/ui/Ask.svelte';
  import Header from '$lib/ui/Header.svelte';
  import Intro from '$lib/ui/Intro.svelte';
  import Onboarding from '$lib/ui/Onboarding.svelte';
  import Tour from '$lib/ui/Tour.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Logo from '$lib/ui/Logo.svelte';
  import Toasts from '$lib/ui/Toasts.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import { count } from '$lib/fmt';
  import { loadMe, net, session } from '$lib/session.svelte';

  let { children } = $props();

  const AUTH = ['/login', '/join', '/register', '/forgot-password', '/reset-password', '/verify-email', '/legal'];
  const path = $derived(page.url.pathname);
  const auth = $derived(AUTH.some((p) => path.startsWith(p)));
  const bare = $derived(auth || path.startsWith('/watch') || path.startsWith('/room'));

  $effect(() => {
    if (auth || session.me) {
      session.loading = false;
      return;
    }
    loadMe().then((me) => {
      const here = location.pathname + location.search;
      if (!me && !AUTH.some((p) => location.pathname.startsWith(p))) {
        goto(`/login?next=${encodeURIComponent(here)}`, { replace: true });
      }
    });
  });

  $effect(() => {
    document.title = session.me?.site.name ?? 'Nova';
  });

  // Esc никогда не выводит окно приложения на Mac из полноэкранного режима (необработанный Esc
  // WebKit отдаёт системе). Окна и меню закрываются своими обработчиками, плеер - выходит в сервис.
  $effect(() => {
    const hold = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || e.defaultPrevented) return;
      e.preventDefault();
      // Без стандартного действия окно <dialog> само не закроется - закрываем верхнее.
      const open = [...document.querySelectorAll<HTMLDialogElement>('dialog[open]')].pop();
      open?.dispatchEvent(new Event('cancel', { cancelable: true })) && open.close();
    };
    window.addEventListener('keydown', hold);
    return () => window.removeEventListener('keydown', hold);
  });

  // Навигация без искусственной задержки; появление новой страницы задано в app.css.
  const leaving = false;
  $effect(() => applyMotion());

  // Окно приложения хоста: перетаскивают файлы - подсказка, бросили - что сделано.
  let dragging = $state(false);
  $effect(() => {
    if (!session.me?.window) return;
    const drag = (e: Event) => (dragging = !!(e as CustomEvent<boolean>).detail);
    const drop = (e: Event) => {
      const d = (e as CustomEvent<{ folders: string[]; files: number; skipped: number }>).detail;
      const parts = [];
      if (d.folders.length) parts.push(`подключено ${count(d.folders.length, 'папка', 'папки', 'папок')}`);
      if (d.files) parts.push(`копируем ${count(d.files, 'файл', 'файла', 'файлов')}`);
      if (d.skipped && !parts.length) parts.push('это уже на сайте или не видео');
      toast(parts.join(', ').replace(/^./, (c) => c.toUpperCase()) || 'Ничего не добавлено', 5000);
    };
    addEventListener('fsfy-drag', drag);
    addEventListener('fsfy-drop', drop);
    return () => {
      removeEventListener('fsfy-drag', drag);
      removeEventListener('fsfy-drop', drop);
    };
  });

  const full = $derived(/^\/chats\/\d/.test(path));
</script>

{#if navigating.to}<div class="navigation-progress" role="status" aria-label="Открываем страницу"></div>{/if}
<Ask />
<Toasts />

{#if dragging}
  <div class="dropping" aria-hidden="true">
    <div>
      <Icon name="download" size={40} />
      <b>Отпустите, чтобы добавить</b>
      <span>Папки подключатся как есть, файлы скопируются в свою папку</span>
    </div>
  </div>
{/if}

{#if net.down}
  <div class="down" role="status">
    <Icon name="offline" size={18} /> Сервер Nova сейчас недоступен - фильмы вернутся, когда он включится
  </div>
{/if}

{#if bare}
  {@render children()}
{:else if session.me}
  <Intro />
  <Onboarding />
  <Tour />
  <Header />
  <main class:leaving>{@render children()}</main>
  {#if !full}
    <footer>
      <div class="top">
        <a href="/" aria-label="На главную"><Logo /></a>
        <nav aria-label="Ссылки">
          <a href="/?tab=movies">Фильмы</a>
          <a href="/?tab=series">Сериалы</a>
          <a href="/?tab=anime">Аниме</a>
          <a href="/discover">Обзор</a>
          <a href="/lists">Списки</a>
          <a href="/people">Люди</a>
          <a href="/chats">Чаты</a>
          <a href="/profile">Профиль</a>
        </nav>
      </div>
      <p class="faint">
        © {new Date().getFullYear()} {session.me.site.name} - хорошие истории становятся ближе · {session.me.site.version}
      </p>
      <p class="legal-links"><a href="/legal/terms">Правила использования</a><a href="/legal/privacy">Конфиденциальность</a></p>
    </footer>
  {/if}
{:else if session.loading}
  <div class="boot"><Loading label="Готовим ваш кинозал" /></div>
{/if}

<style>
  .navigation-progress { position:fixed;z-index:1000;top:0;left:0;width:40%;height:2px;background:#fff;box-shadow:0 0 8px #ffffff50;animation:navigation-slide .7s ease-in-out infinite alternate;pointer-events:none; }
  @keyframes navigation-slide { from {transform:translateX(-30%)} to {transform:translateX(180%)} }
  /* Подвал всегда ниже первого экрана: пока страница грузится, он не прыгает вниз (CLS). */
  main {
    min-height: 100dvh;
    transition:
      opacity 0.12s ease-in,
      transform 0.12s ease-in;
  }
  main.leaving {
    opacity: 0;
    transform: translateY(-6px);
  }
  footer {
    display: grid;
    gap: 18px;
    max-width: 1060px;
    margin: 64px auto 0;
    padding: 30px var(--gutter) calc(28px + var(--tabbar));
    font-size: 13px;
  }
  footer .top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 16px 28px;
  }
  footer nav {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 26px;
  }
  footer nav a {
    font-weight: 700;
    font-size: 12.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    transition: opacity 0.15s;
  }
  footer nav a:hover {
    opacity: 0.7;
  }
  footer p {
    margin: 0;
    text-align: center;
    font-size: 11.5px;
  }
  .legal-links { display: flex; flex-wrap: wrap; justify-content: center; gap: 12px 24px; color: var(--muted); }
  .dropping {
    position: fixed;
    inset: 0;
    z-index: 95;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(0, 0, 0, 0.72);
    animation: fade 0.15s ease-out;
  }
  .dropping div {
    display: grid;
    justify-items: center;
    gap: 10px;
    padding: 40px 48px;
    border: 2px dashed rgba(255, 255, 255, 0.6);
    border-radius: 24px;
    text-align: center;
  }
  .dropping b {
    font-size: 22px;
  }
  .dropping span {
    color: var(--muted);
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  main { animation: service-entry .35s ease-out both; }
  @keyframes service-entry { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: none; } }
  @media (prefers-reduced-motion: reduce) { main { animation: none; } }
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
    bottom: calc(20px + var(--tabbar));
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
