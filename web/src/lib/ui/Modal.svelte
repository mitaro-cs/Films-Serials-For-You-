<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let { open = $bindable(false), title, wide = false, sheet = false, children }: { open: boolean; title: string; wide?: boolean; sheet?: boolean; children: Snippet } =
    $props();
  let dialog: HTMLDialogElement | undefined = $state();
  // Закрывается плавно: сначала анимация, потом close(); содержимое живёт до конца анимации.
  let closing = $state(false);

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) {
      closing = false;
      dialog.showModal();
    }
    if (!open && dialog.open && !closing) {
      closing = true;
      const d = dialog;
      d.close();
      closing = false;
    }
  });

  // Esc и клик по затемнению - тоже через анимацию (слой Esc в +layout видит отмену и не закрывает).
  function cancel(e: Event) {
    e.preventDefault();
    open = false;
  }
</script>

<dialog bind:this={dialog} class:wide class:sheet class:closing onclose={() => (open = false)} oncancel={cancel} aria-label={title}>
  {#if open || closing}
    <header>
      <h2>{title}</h2>
      <button class="btn ghost sm icon" aria-label="Закрыть" onclick={() => (open = false)}><Icon name="close" size={18} /></button>
    </header>
    <div class="content">{@render children()}</div>
  {/if}
</dialog>

<style>
  @media (max-width:640px) {
    dialog.sheet { position:fixed; inset:auto 0 0; margin:0; width:100%; max-width:none; max-height:85dvh; border:0; border-radius:18px 18px 0 0; padding-bottom:env(safe-area-inset-bottom); }
    dialog.sheet header { justify-content:center; }
    dialog.sheet header h2 { text-align:center; }
  }
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
  /* Появление: масштаб и прозрачность - без сдвига вёрстки под окном. */
  dialog[open] {
    animation: pop 0.22s cubic-bezier(0.2, 0.9, 0.3, 1.2);
  }
  dialog[open]::backdrop {
    animation: fade 0.2s ease-out;
  }
  dialog.closing {
    animation: pop-out 0.18s ease-in forwards;
  }
  dialog.closing::backdrop {
    animation: fade-out 0.18s ease-in forwards;
  }
  @keyframes pop-out {
    to {
      opacity: 0;
      transform: translateY(6px) scale(0.97);
    }
  }
  @keyframes fade-out {
    to {
      opacity: 0;
    }
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
