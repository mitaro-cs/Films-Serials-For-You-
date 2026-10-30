<script lang="ts">
  import { motionOn } from '$lib/motion';

  // Заставка при входе в сервис: знак Nova проявляется, по нему пробегает световая черта, затем две
  // шторы расходятся вверх и вниз и открывается кинозал. Раз за сессию вкладки; анимации выключены -
  // сразу. Клики не держит (pointer-events: none) - под ней уже работающая страница. «Экскурсия» из
  // меню заставку не повторяет - она показывает сам сервис.
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
    // Автотесты (Playwright) - без заставки: она на пару секунд закрывает страницу от проверок вёрстки.
    if (seen || !motionOn() || navigator.webdriver) return;
    show = true;
    const t = setTimeout(() => (show = false), 2300);
    return () => clearTimeout(t);
  });
</script>

{#if show}
  <div class="intro" aria-hidden="true">
    <i class="half up"></i>
    <i class="half down"></i>
    <div class="mark">
      <img src="/logo.svg" alt="" width="132" height="67" />
      <i class="sweep"></i>
    </div>
  </div>
{/if}

<style>
  .intro {
    position: fixed;
    inset: 0;
    z-index: 2000;
    display: grid;
    place-items: center;
    pointer-events: none;
    overflow: hidden;
  }
  .half {
    position: absolute;
    left: 0;
    right: 0;
    height: 50.2%;
    background: #000;
    animation-duration: 0.8s;
    animation-delay: 1.45s;
    animation-timing-function: cubic-bezier(0.7, 0, 0.2, 1);
    animation-fill-mode: both;
  }
  .up {
    top: 0;
    animation-name: up;
  }
  .down {
    bottom: 0;
    animation-name: down;
  }
  @keyframes up {
    to {
      transform: translateY(-101%);
    }
  }
  @keyframes down {
    to {
      transform: translateY(101%);
    }
  }
  .mark {
    position: relative;
    z-index: 1;
    overflow: hidden;
    animation:
      mark-in 1s cubic-bezier(0.2, 0.8, 0.2, 1) both,
      mark-out 0.5s ease 1.35s both;
  }
  img {
    display: block;
    width: 132px;
    height: auto;
  }
  .sweep {
    position: absolute;
    inset: 0;
    background: linear-gradient(100deg, transparent 30%, #ffffffb3 50%, transparent 70%);
    transform: translateX(-120%);
    mix-blend-mode: overlay;
    animation: sweep 0.9s ease 0.6s both;
  }
  @keyframes mark-in {
    0% {
      opacity: 0;
      transform: scale(0.8);
    }
    100% {
      opacity: 1;
      transform: scale(1);
    }
  }
  @keyframes mark-out {
    to {
      opacity: 0;
      transform: scale(1.12);
    }
  }
  @keyframes sweep {
    to {
      transform: translateX(120%);
    }
  }
</style>
