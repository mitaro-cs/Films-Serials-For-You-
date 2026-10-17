<script lang="ts">
  import { clock, size } from '$lib/fmt';
  import Icon from '$lib/ui/Icon.svelte';
  import type { Status } from './types';

  let { status, reload }: { status: Status; reload: () => void } = $props();
  let copied = $state(false);

  $effect(() => {
    const t = setInterval(reload, 5000);
    return () => clearInterval(t);
  });

  async function copy() {
    if (!status.publicUrl) return;
    await navigator.clipboard?.writeText(status.publicUrl).catch(() => {});
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }
</script>

<div class="tiles">
  <div class="tile dark">
    <span>На сайте</span>
    <b>{status.titles}</b>
    <small>фильмов и сериалов</small>
  </div>
  <div class="tile"><span>Файлов</span><b>{status.files}</b><small>{size(status.size)}</small></div>
  <div class="tile">
    <span>Описания</span>
    <b>{status.metaPending ? status.metaPending : '✓'}</b>
    <small>{status.metaPending ? (status.meta.running ? 'загружаются…' : 'ждут загрузки') : 'всё загружено'}</small>
  </div>
  <div class="tile"><span>Сейчас смотрят</span><b>{status.watching.length}</b><small>{status.watching.length ? 'человек' : 'никого'}</small></div>
</div>

{#if status.meta.error}
  <p class="note warn"><Icon name="info" size={16} /> Описания: {status.meta.error}</p>
{/if}
{#if status.scan.error}
  <p class="note warn"><Icon name="info" size={16} /> Папки: {status.scan.error}</p>
{/if}
{#if !status.ffmpeg}
  <p class="note">
    <Icon name="info" size={16} />
    <span>
      Установите <b>ffmpeg</b>, чтобы смотрели MKV, AVI и фильмы с несколькими озвучками:
      Mac — <code>brew install ffmpeg</code>, Windows — <code>winget install ffmpeg</code>. Перезапуск не нужен — укажите путь в «Настройках» или
      просто подождите пару минут.
    </span>
  </p>
{/if}

<section class="card pane">
  <h3><Icon name="link" size={18} /> Адрес для зрителей</h3>
  {#if status.publicUrl}
    <div class="url">
      <code>{status.publicUrl}</code>
      <button class="btn sm" onclick={copy}><Icon name={copied ? 'check' : 'copy'} size={15} /> {copied ? 'Скопировано' : 'Копировать'}</button>
    </div>
  {:else}
    <p class="muted">Доступ из интернета выключен — откройте вкладку «Доступ».</p>
  {/if}
</section>

<section class="card pane">
  <h3><Icon name="pulse" size={18} /> Сейчас смотрят</h3>
  {#if status.watching.length}
    <ul>
      {#each status.watching as w, i (i)}
        <li>
          <b>{w.user}</b>
          <span>{w.title}{w.episode ? ` · ${w.episode}` : ''}</span>
          <span class="faint">{clock(w.position)} / {clock(w.duration)}</span>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted">Никто не смотрит.</p>
  {/if}
</section>

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 12px;
    margin-bottom: 18px;
  }
  .tile {
    display: grid;
    gap: 2px;
    padding: 18px;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .tile.dark {
    background: #fff;
    color: var(--on-accent);
    border: 0;
  }
  .tile.dark :global(.muted) {
    color: #52525b;
  }
  .tile span {
    font-size: 13px;
    opacity: 0.8;
  }
  .tile b {
    font-size: 30px;
    line-height: 1.1;
  }
  .tile small {
    font-size: 12px;
    opacity: 0.75;
  }
  .note {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 12px 14px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    border: 1px solid var(--line);
    font-size: 14px;
    margin: 0 0 12px;
  }
  .note.warn {
    border-color: rgba(255, 184, 77, 0.4);
    color: var(--warn);
  }
  code {
    padding: 1px 6px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.35);
    font-size: 13px;
  }
  .pane {
    padding: 18px 20px;
    margin-bottom: 14px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
    margin-bottom: 12px;
  }
  .url {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .url code {
    font-size: 15px;
    padding: 6px 10px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  li {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: 14px;
  }
  p {
    margin: 0;
  }
</style>
