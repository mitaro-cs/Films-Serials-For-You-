<script lang="ts">
  import { toasts } from './toast.svelte';
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="t">{t.text}</div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    z-index: 90;
    left: 50%;
    bottom: calc(20px + var(--tabbar));
    transform: translateX(-50%);
    display: grid;
    gap: 8px;
    justify-items: center;
    width: max-content;
    max-width: calc(100vw - 32px);
    pointer-events: none;
  }
  .t {
    padding: 11px 18px;
    border-radius: 999px;
    background: #fff;
    color: #0a0a0a;
    font-size: 14px;
    font-weight: 600;
    box-shadow: 0 16px 40px -12px rgba(0, 0, 0, 0.8);
    animation: up 0.3s cubic-bezier(0.2, 0.9, 0.3, 1.2) both;
  }
  @keyframes up {
    from {
      opacity: 0;
      transform: translateY(14px) scale(0.96);
    }
  }
</style>
