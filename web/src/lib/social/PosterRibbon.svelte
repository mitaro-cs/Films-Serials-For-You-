<script lang="ts">
  import { placeholder } from '$lib/ui/art';

  // Обложка профиля — лента постеров просмотренного: два наклонённых ряда плывут навстречу.
  // Только transform: видеокарте легко. Мало постеров — повторяем; нет — мягкий градиент цвета.
  let { posters, accent = null }: { posters: string[]; accent?: string | null } = $props();

  const PER = 10;
  const rows = $derived(
    posters.length ? [0, 1].map((r) => Array.from({ length: PER }, (_, i) => posters[(i * 2 + r * 5) % posters.length])) : [],
  );
</script>

<div class="ribbon" style:--acc={accent ?? '#3a3a3c'} aria-hidden="true">
  {#if rows.length}
    <div class="tilt">
      {#each rows as row, r (r)}
        <div class="row" class:rev={r === 1}>
          {#each [0, 1] as half (half)}
            {#each row as src, i (`${half}-${i}`)}
              <span class="tile" style:background={placeholder(String(i + r))}><img {src} alt="" decoding="async" loading="lazy" /></span>
            {/each}
          {/each}
        </div>
      {/each}
    </div>
  {/if}
  <div class="shade"></div>
</div>

<style>
  .ribbon {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background:
      radial-gradient(120% 140% at 0% 0%, color-mix(in srgb, var(--acc) 80%, transparent), transparent 60%),
      linear-gradient(135deg, color-mix(in srgb, var(--acc) 45%, #111), #111);
  }
  .tilt {
    position: absolute;
    inset: -40% -10%;
    display: grid;
    align-content: center;
    gap: 12px;
    transform: rotate(-8deg);
  }
  .row {
    display: flex;
    gap: 12px;
    width: max-content;
    animation: drift 80s linear infinite;
  }
  .row.rev {
    animation-direction: reverse;
    animation-duration: 95s;
  }
  @keyframes drift {
    to {
      transform: translateX(-50%);
    }
  }
  .tile {
    position: relative;
    flex: none;
    width: 112px;
    aspect-ratio: 2 / 3;
    border-radius: 10px;
    overflow: hidden;
  }
  .tile img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Снизу темнее — имя и аватар читаются; цвет профиля чуть подкрашивает ленту. */
  .shade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, rgba(0, 0, 0, 0.75), rgba(0, 0, 0, 0.15) 60%),
      linear-gradient(90deg, color-mix(in srgb, var(--acc) 35%, transparent), transparent 70%);
  }
  @media (prefers-reduced-motion: reduce) {
    .row {
      animation-name: none !important;
    }
  }
</style>
