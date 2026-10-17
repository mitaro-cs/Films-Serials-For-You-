<script lang="ts">
  import type { Snippet } from 'svelte';
  import Logo from './Logo.svelte';

  let { title, subtitle, children }: { title: string; subtitle?: string; children: Snippet } = $props();
</script>

<div class="shell">
  <div class="orb a"></div>
  <div class="orb b"></div>
  <div class="strip" aria-hidden="true">
    {#each Array(14) as _, i (i)}<span style:--d="{(i % 7) * 0.6}s" style:--h="{(i * 47) % 360}"></span>{/each}
  </div>
  <div class="box card">
    <div class="logo"><Logo /></div>
    <h1>{title}</h1>
    {#if subtitle}<p class="muted">{subtitle}</p>{/if}
    {@render children()}
  </div>
</div>

<style>
  .shell {
    position: relative;
    min-height: 100dvh;
    display: grid;
    place-items: center;
    padding: 24px 16px;
    overflow: hidden;
  }
  .orb {
    position: absolute;
    width: 60vmax;
    height: 60vmax;
    border-radius: 50%;
    filter: blur(90px);
    opacity: 0.35;
  }
  .a {
    background: #3a3a3c;
    top: -30vmax;
    right: -20vmax;
  }
  .b {
    background: #52525b;
    bottom: -35vmax;
    left: -25vmax;
    opacity: 0.22;
  }
  .strip {
    position: absolute;
    inset: auto -10% 6% -10%;
    display: flex;
    gap: 14px;
    transform: rotate(-8deg);
    opacity: 0.25;
  }
  .strip span {
    flex: none;
    width: 110px;
    aspect-ratio: 2 / 3;
    border-radius: 10px;
    /* Ряд билетов в цвет постеров — как карточки в каталоге. */
    background: linear-gradient(180deg, hsl(var(--h) 30% 32%) 0 58%, hsl(var(--h) 25% 22%) 58%);
    animation: float 6s ease-in-out infinite;
    animation-delay: var(--d);
  }
  @keyframes float {
    50% {
      transform: translateY(-14px);
    }
  }
  .box {
    position: relative;
    width: min(420px, 100%);
    padding: 34px 30px 30px;
    background: rgba(18, 18, 20, 0.86);
    backdrop-filter: blur(20px);
    box-shadow: 0 40px 80px -30px rgba(0, 0, 0, 0.8);
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
</style>
