<script lang="ts">
  // Фон из постеров просмотренного: наклонённые колонки плавно плывут вверх, каждая своей
  // скоростью. `page` - на весь экран за страницей профиля; иначе - внутри своего блока (превью).
  // Постеров мало - между ними цветные плитки, чтобы стена не была из одного фильма.
  // Только transform: видеокарте легко.
  let { posters, accent = null, page = false }: { posters: string[]; accent?: string | null; page?: boolean } = $props();

  const COLS = $derived(page ? 12 : 8);
  const PER = 6;
  const pool = $derived(
    posters.length >= 8 ? posters : [...posters, ...Array<null>(Math.max(0, 8 - posters.length)).fill(null)],
  );
  const cols = $derived(
    Array.from({ length: COLS }, (_, c) => Array.from({ length: PER }, (_, i) => pool[(c * PER + i * 5 + c * 3) % pool.length] ?? null)),
  );
</script>

<div class="wall" class:page style:--acc={accent ?? '#3a3a3c'} aria-hidden="true">
  <div class="tilt">
    {#each cols as col, c (c)}
      <div class="col" style:--speed="{70 + ((c * 17) % 5) * 10}s" style:--delay="{-((c * 23) % 50)}s">
        {#each [0, 1] as half (half)}
          {#each col as src, i (`${half}-${i}`)}
            <span class="tile" style:--h={(c * 61 + i * 37) % 360}>{#if src}<img {src} alt="" decoding="async" loading="lazy" />{/if}</span>
          {/each}
        {/each}
      </div>
    {/each}
  </div>
  <div class="veil"></div>
</div>

<style>
  .wall {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: #050506;
  }
  .wall.page {
    position: fixed;
    z-index: -1;
  }
  .tilt {
    position: absolute;
    inset: -40% -15%;
    display: flex;
    justify-content: center;
    gap: 16px;
    transform: rotate(-10deg);
    opacity: 0;
    animation: wall-in 1.2s 0.1s ease-out forwards;
  }
  .page .tilt {
    animation-name: wall-in-dim;
  }
  @keyframes wall-in {
    to {
      opacity: 0.7;
    }
  }
  @keyframes wall-in-dim {
    to {
      opacity: 0.45;
    }
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 16px;
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
    width: clamp(96px, 9vw, 150px);
    aspect-ratio: 2 / 3;
    border-radius: 10px;
    overflow: hidden;
    background: linear-gradient(160deg, hsl(var(--h) 40% 24%), hsl(calc(var(--h) + 40) 35% 11%));
  }
  .tile img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Цвет профиля подкрашивает стену; к низу и к центру темнее - текст читается. */
  .veil {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(ellipse 70% 60% at 50% 45%, rgba(5, 5, 6, 0.55), rgba(5, 5, 6, 0.15) 70%),
      linear-gradient(180deg, color-mix(in srgb, var(--acc) 25%, transparent), transparent 35%, rgba(5, 5, 6, 0.85) 85%);
  }
  :global(.no-motion) .col {
    animation-name: none !important;
  }
</style>
