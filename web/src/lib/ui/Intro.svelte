<script lang="ts">
  import { motionOn } from '$lib/motion';

  // Заставка при входе в сервис (вариант B, выбор владельца): буквы NOVA проявляются по очереди из
  // размытия, затем чёрный экран растворяется, а страница под ним чуть «отъезжает» (app.css, `intro-reveal`).
  // Раз за сессию вкладки; анимации выключены - сразу. Клики не держит (pointer-events: none).
  const KEY = 'nova-intro';
  const LETTERS = ['N', 'O', 'V', 'A'];
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
    const root = document.documentElement;
    root.classList.add('intro-reveal');
    const t = setTimeout(() => (show = false), 2500);
    const r = setTimeout(() => root.classList.remove('intro-reveal'), 3400);
    return () => {
      clearTimeout(t);
      clearTimeout(r);
      root.classList.remove('intro-reveal');
    };
  });
</script>

{#if show}
  <div class="intro" aria-hidden="true">
    <div class="word">
      {#each LETTERS as l, i (i)}<span style:--i={i}>{l}</span>{/each}
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
    background: #000;
    pointer-events: none;
    animation: intro-out 0.8s ease 1.7s both;
  }
  .word {
    display: flex;
    gap: 0.06em;
    color: #fff;
    font-size: clamp(44px, 8vw, 96px);
    font-weight: 800;
  }
  span {
    display: inline-block;
    animation: letter 0.8s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(var(--i) * 150ms);
  }
  @keyframes letter {
    from {
      opacity: 0;
      filter: blur(14px);
      transform: translateY(14px);
    }
  }
  @keyframes intro-out {
    to {
      opacity: 0;
    }
  }
</style>
