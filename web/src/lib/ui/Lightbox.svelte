<script lang="ts">
  import Icon from './Icon.svelte';

  let { images, index = $bindable<number | null>() }: { images: string[]; index: number | null } = $props();

  function go(d: number) {
    if (index === null) return;
    index = (index + d + images.length) % images.length;
  }

  function key(e: KeyboardEvent) {
    if (index === null) return;
    if (e.key === 'Escape') index = null;
    if (e.key === 'ArrowRight') go(1);
    if (e.key === 'ArrowLeft') go(-1);
  }
</script>

<svelte:window onkeydown={key} />

{#if index !== null}
  <div class="lb" role="dialog" aria-modal="true" aria-label="Галерея">
    <button class="bg" aria-label="Закрыть" onclick={() => (index = null)}></button>
    <img src={images[index]} alt="" />
    <button class="btn icon close" aria-label="Закрыть" onclick={() => (index = null)}><Icon name="close" /></button>
    {#if images.length > 1}
      <button class="btn icon nav l" aria-label="Назад" onclick={() => go(-1)}><Icon name="back" /></button>
      <button class="btn icon nav r" aria-label="Дальше" onclick={() => go(1)}><Icon name="right" /></button>
      <span class="n">{index + 1} / {images.length}</span>
    {/if}
  </div>
{/if}

<style>
  .lb {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: grid;
    place-items: center;
    animation: in 0.2s ease;
  }
  @keyframes in {
    from {
      opacity: 0;
    }
  }
  .bg {
    position: absolute;
    inset: 0;
    border: 0;
    background: rgba(5, 4, 14, 0.92);
    backdrop-filter: blur(6px);
  }
  img {
    position: relative;
    max-width: 92vw;
    max-height: 86vh;
    border-radius: 12px;
    box-shadow: 0 30px 80px -20px #000;
  }
  .close {
    position: absolute;
    top: 18px;
    right: 18px;
  }
  .nav {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
  }
  .l {
    left: 18px;
  }
  .r {
    right: 18px;
  }
  .n {
    position: absolute;
    bottom: 20px;
    font-size: 13px;
    color: var(--muted);
  }
</style>
