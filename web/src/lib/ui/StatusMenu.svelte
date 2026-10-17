<script lang="ts">
  import { statusMenu, toggleList, toggleWatched } from '$lib/cards.svelte';
  import Icon from './Icon.svelte';

  // Меню статуса у карточки: одно на страницу, ставится под кнопкой и всегда целиком на экране.
  const c = $derived(statusMenu.card);
  let el = $state<HTMLDivElement>();
  let pos = $state({ left: 0, top: 0 });

  // Ставим меню у кнопки; при прокрутке оно едет за ней, ушла с экрана — закрываем.
  function place() {
    if (!statusMenu.anchor || !el) return;
    const r = statusMenu.anchor.getBoundingClientRect();
    if (r.bottom < 0 || r.top > innerHeight) {
      statusMenu.card = null;
      return;
    }
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const pad = 12;
    let left = r.right - w;
    let top = r.bottom + 6;
    if (top + h > innerHeight - pad) top = r.top - h - 6;
    left = Math.min(Math.max(pad, left), innerWidth - w - pad);
    top = Math.min(Math.max(pad, top), innerHeight - h - pad);
    pos = { left, top };
  }

  $effect(() => {
    if (!c || !el) return;
    place();
    el.querySelector<HTMLElement>('button:not(:disabled), a')?.focus({ preventScroll: true });
  });

  $effect(() => {
    if (!c) return;
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      statusMenu.card = null;
      statusMenu.anchor?.focus({ preventScroll: true });
    };
    addEventListener('keydown', key);
    addEventListener('scroll', place, { passive: true, capture: true });
    addEventListener('resize', place);
    return () => {
      removeEventListener('keydown', key);
      removeEventListener('scroll', place, { capture: true });
      removeEventListener('resize', place);
    };
  });

  function run(f: () => unknown) {
    f();
    statusMenu.card = null;
  }
</script>

{#if c}
  <button class="scrim" aria-label="Закрыть" onclick={() => (statusMenu.card = null)}></button>
  <div class="menu" role="menu" aria-label="Статус «{c.title}»" bind:this={el} style:left="{pos.left}px" style:top="{pos.top}px">
    <button role="menuitemcheckbox" aria-checked={c.status === 'want'} disabled={c.status === 'watching' || c.status === 'watched'} onclick={() => run(() => toggleList(c))}>
      <Icon name="bookmark" size={18} /> Хочу посмотреть {#if c.status === 'want'}<Icon name="check" size={16} />{/if}
    </button>
    <a role="menuitem" href={c.resume ? `/watch/${c.resume.fileId}` : `/title/${c.id}`} onclick={() => (statusMenu.card = null)}>
      <Icon name="play" size={18} /> {c.status === 'watching' ? 'Продолжить' : 'Смотреть'}
    </a>
    <button role="menuitemcheckbox" aria-checked={c.status === 'watched'} onclick={() => run(() => toggleWatched(c))}>
      <Icon name="check" size={18} /> Просмотрено {#if c.status === 'watched'}<Icon name="check" size={16} />{/if}
    </button>
    {#if c.status === 'watched'}
      <p>Снять отметку — и смотреть заново с начала.</p>
    {/if}
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 70;
    border: 0;
    background: transparent;
  }
  .menu {
    position: fixed;
    z-index: 71;
    display: grid;
    width: 230px;
    max-width: calc(100vw - 24px);
    padding: 6px;
    border-radius: 14px;
    background: rgba(28, 28, 30, 0.97);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid var(--line);
    box-shadow: 0 20px 50px -10px rgba(0, 0, 0, 0.85);
  }
  button,
  a {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border: 0;
    border-radius: 9px;
    background: none;
    font-size: 14px;
    text-align: left;
    cursor: pointer;
  }
  button :global(svg:last-child:not(:first-child)) {
    margin-left: auto;
  }
  button:hover:not(:disabled),
  a:hover {
    background: var(--surface-2);
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  p {
    margin: 2px 12px 6px;
    font-size: 12px;
    color: var(--muted);
  }
</style>
