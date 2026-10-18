<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let { open = $bindable(false), title, wide = false, children }: { open: boolean; title: string; wide?: boolean; children: Snippet } =
    $props();
  let dialog: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });
</script>

<dialog bind:this={dialog} class:wide onclose={() => (open = false)} aria-label={title}>
  {#if open}
    <header>
      <h2>{title}</h2>
      <button class="btn ghost sm icon" aria-label="Закрыть" onclick={() => (open = false)}><Icon name="close" size={18} /></button>
    </header>
    <div class="content">{@render children()}</div>
  {/if}
</dialog>

<style>
  dialog {
    width: min(520px, calc(100vw - 24px));
    max-height: calc(100dvh - 48px);
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 18px;
    background: #1c1c1e;
    color: var(--text);
    box-shadow: 0 40px 100px -20px #000;
    overflow: auto;
  }
  /* Появление: масштаб и прозрачность — без сдвига вёрстки под окном. */
  dialog[open] {
    animation: pop 0.22s cubic-bezier(0.2, 0.9, 0.3, 1.2);
  }
  dialog[open]::backdrop {
    animation: fade 0.2s ease-out;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.97);
    }
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  dialog.wide {
    width: min(860px, calc(100vw - 24px));
  }
  dialog::backdrop {
    background: rgba(0, 0, 0, 0.7);
  }
  header {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 16px 12px 22px;
    background: #1c1c1e;
  }
  h2 {
    font-size: 18px;
  }
  .content {
    padding: 4px 22px 22px;
  }
</style>
