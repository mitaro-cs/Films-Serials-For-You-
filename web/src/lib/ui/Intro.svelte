<script lang="ts">
  import { motionOn } from '$lib/motion';

  // Заставка при входе в сервис: знак Nova проявляется и чуть приближается, затем всё растворяется
  // и открывается кинозал. Раз за сессию вкладки; анимации выключены - сразу. Клики не держит
  // (pointer-events: none) - под ней уже работающая страница.
  const KEY = 'nova-intro';
  let show = $state(false);
  $effect(() => {
    let seen = false;
    try {
      seen = sessionStorage.getItem(KEY) === '1';
      sessionStorage.setItem(KEY, '1');
    } catch {
      seen = false;
    }
    // Автотесты (Playwright) - без заставки: она на полторы секунды закрывает страницу от проверок вёрстки.
    if (seen || !motionOn() || navigator.webdriver) return;
    show = true;
    const t = setTimeout(() => (show = false), 1500);
    return () => clearTimeout(t);
  });
</script>

{#if show}
  <div class="intro" aria-hidden="true">
    <img src="/logo.svg" alt="" width="120" height="61" />
  </div>
{/if}

<style>
  .intro {
    position: fixed;
    inset: 0;
    z-index: 2000;
    display: grid;
    place-items: center;
    background: #000;
    pointer-events: none;
    animation: intro-out 0.5s ease 1s both;
  }
  img {
    width: 120px;
    height: auto;
    animation: intro-mark 1.1s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  @keyframes intro-mark {
    0% {
      opacity: 0;
      transform: scale(0.82);
    }
    45% {
      opacity: 1;
      transform: scale(1);
    }
    100% {
      opacity: 1;
      transform: scale(1.08);
    }
  }
  @keyframes intro-out {
    to {
      opacity: 0;
    }
  }
</style>
