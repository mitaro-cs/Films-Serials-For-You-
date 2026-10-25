<script lang="ts">
  import type { Snippet } from 'svelte';
  import Logo from './Logo.svelte';

  // Вход и регистрация: за формой - стена постеров популярного с IMDb, колонки плавно плывут
  // вверх (каждая со своей скоростью), сверху медленно дышат цветные пятна. Только
  // transform/opacity - видеокарте легко. Постеров ещё нет - цветные заглушки в том же ритме.
  let { title, subtitle, children }: { title: string; subtitle?: string; children: Snippet } = $props();

  const COLS = 14;
  const PER = 6;
  let posters = $state<string[]>([]);

  $effect(() => {
    fetch('/api/site/wall', { headers: { 'X-Fsfy': '1' } })
      .then((r) => (r.ok ? r.json() : []))
      .then((list: string[]) => (posters = list), () => {});
  });

  // Колонка c - свои постеры по кругу; дважды подряд, чтобы бегущая лента была бесшовной.
  const cols = $derived(
    Array.from({ length: COLS }, (_, c) =>
      Array.from({ length: PER }, (_, i) => (posters.length ? posters[(c * PER + i * 5 + c) % posters.length] : null)),
    ),
  );
</script>

<div class="shell">
  <div class="wall" aria-hidden="true">
    {#each cols as col, r (r)}
      <div class="col" style:--speed="{60 + ((r * 17) % 5) * 9}s" style:--delay="{-((r * 23) % 40)}s">
        {#each [0, 1] as half (half)}
          {#each col as src, i (`${half}-${i}`)}
            <span class="tile" style:--h={(r * 61 + i * 37) % 360}>
              {#if src}<img {src} alt="" decoding="async" width="342" height="513" />{/if}
            </span>
          {/each}
        {/each}
      </div>
    {/each}
  </div>
  <div class="glow a" aria-hidden="true"></div>
  <div class="glow b" aria-hidden="true"></div>
  <div class="veil" aria-hidden="true"></div>

  <div class="box card">
    <div class="logo"><Logo /></div>
    <h1>{title}</h1>
    {#if subtitle}<p class="muted">{subtitle}</p>{/if}
    {@render children()}
    <nav class="legal" aria-label="Документы"><a href="/legal/terms">Правила использования</a><a href="/legal/privacy">Конфиденциальность</a></nav>
  </div>
</div>

<style>
  .legal { display: flex; flex-wrap: wrap; justify-content: center; gap: 10px 18px; margin-top: 24px; color: var(--muted); font-size: 12px; }
  .shell {
    position: relative;
    min-height: 100dvh;
    display: grid;
    place-items: center;
    padding: 24px 16px;
    overflow: hidden;
    isolation: isolate;
    background: #050506;
  }

  /* Стена: наклонённые ряды постеров на весь экран. */
  .wall {
    position: absolute;
    inset: -40% -15%;
    z-index: -3;
    display: flex;
    justify-content: center;
    gap: 18px;
    transform: rotate(-12deg);
    opacity: 0;
    animation: wall-in 1.2s 0.1s ease-out forwards;
  }
  @keyframes wall-in {
    to {
      opacity: 0.55;
    }
  }
  /* Колонки плывут вверх, каждая своей скоростью и со своего места - стена «живая». */
  .col {
    display: flex;
    flex-direction: column;
    gap: 18px;
    height: max-content;
    animation: drift var(--speed) linear infinite;
    animation-delay: var(--delay);
  }
  @keyframes drift {
    to {
      transform: translateY(-50%);
    }
  }
  .tile {
    position: relative;
    flex: none;
    width: clamp(110px, 11vw, 170px);
    aspect-ratio: 2 / 3;
    border-radius: 12px;
    overflow: hidden;
    background: linear-gradient(160deg, hsl(var(--h) 45% 26%), hsl(calc(var(--h) + 40) 40% 12%));
  }
  .tile img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  /* Цветные пятна плывут медленно - градиентом и transform, без filter: blur. */
  .glow {
    position: absolute;
    z-index: -2;
    width: 70vmax;
    height: 70vmax;
    border-radius: 50%;
    pointer-events: none;
  }
  .glow.a {
    top: -35vmax;
    left: -25vmax;
    background: radial-gradient(closest-side, rgba(255, 140, 60, 0.32), transparent);
    animation: float-a 22s ease-in-out infinite alternate;
  }
  .glow.b {
    bottom: -40vmax;
    right: -25vmax;
    background: radial-gradient(closest-side, rgba(120, 90, 255, 0.3), transparent);
    animation: float-b 26s ease-in-out infinite alternate;
  }
  @keyframes float-a {
    to {
      transform: translate(18vmax, 12vmax) scale(1.15);
    }
  }
  @keyframes float-b {
    to {
      transform: translate(-16vmax, -14vmax) scale(1.2);
    }
  }

  /* Вокруг формы темнее - текст читается на любом постере. */
  .veil {
    position: absolute;
    inset: 0;
    z-index: -1;
    background:
      radial-gradient(ellipse 60% 70% at 50% 50%, rgba(5, 5, 6, 0.86) 0%, rgba(5, 5, 6, 0.5) 55%, rgba(5, 5, 6, 0.15) 100%),
      linear-gradient(180deg, rgba(5, 5, 6, 0.6), transparent 25%, transparent 75%, rgba(5, 5, 6, 0.7));
  }

  .box {
    position: relative;
    width: min(420px, 100%);
    padding: 34px 30px 30px;
    background: rgba(18, 18, 20, 0.9);
    box-shadow:
      0 40px 80px -30px rgba(0, 0, 0, 0.9),
      inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    animation: box-in 0.6s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  @keyframes box-in {
    from {
      opacity: 0;
      transform: translateY(18px) scale(0.98);
    }
  }
  .logo {
    margin-bottom: 26px;
  }
  h1 {
    font-size: 26px;
  }
  p {
    margin: 8px 0 0;
  }
  .box :global(form) {
    display: grid;
    gap: 14px;
    margin-top: 24px;
  }

  :global(.no-motion) :is(.col, .glow) {
    animation-name: none !important;
  }
</style>
