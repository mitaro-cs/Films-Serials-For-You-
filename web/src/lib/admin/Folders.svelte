<script lang="ts">
  import { api, del, errorText, post } from '$lib/api';
  import { ago, count, size } from '$lib/fmt';
  import { ask } from '$lib/ui/dialogs.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Inbox from './Inbox.svelte';
  import type { Status } from './types';

  let { status, window: inWindow, reload }: { status: Status; window: boolean; reload: () => void } = $props();

  interface Lib {
    id: number;
    path: string;
    kind: 'auto' | 'movies' | 'series';
    files: number;
    size: number;
    exists: boolean;
  }
  let libs = $state<Lib[]>([]);
  let path = $state('');
  let kind = $state<'auto' | 'movies' | 'series'>('auto');
  let error = $state('');
  let busy = $state(false);

  const KINDS = { auto: 'Фильмы и сериалы', movies: 'Только фильмы', series: 'Только сериалы' };

  function load() {
    api<Lib[]>('/api/admin/libraries').then((l) => (libs = l), () => {});
  }
  $effect(load);
  $effect(() => {
    if (!status.scan.running && !status.import?.running) return;
    const t = setInterval(() => {
      load();
      reload();
    }, 1500);
    return () => clearInterval(t);
  });

  async function pick() {
    error = '';
    try {
      const r = await post<{ path: string | null }>('/api/admin/libraries/pick');
      if (r.path) path = r.path;
    } catch (e) {
      error = errorText(e);
    }
  }

  async function add(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      await post('/api/admin/libraries', { path, kind });
      path = '';
      load();
      setTimeout(reload, 300);
    } catch (x) {
      error = errorText(x);
    } finally {
      busy = false;
    }
  }

  async function remove(l: Lib) {
    if (!(await ask(`Убрать папку «${l.path}» с сайта? Файлы на диске не удаляются.`, 'Убрать', true))) return;
    await del(`/api/admin/libraries/${l.id}`).catch(() => {});
    load();
  }

  async function rescan() {
    await post('/api/admin/scan').catch(() => {});
    setTimeout(reload, 300);
  }
</script>

<Inbox {status} window={inWindow} {reload} />

<form class="card pane add" onsubmit={add}>
  <h3><Icon name="folder" size={18} /> Подключить папку как есть</h3>
  <p class="muted">Своя коллекция на этом компьютере или внешнем диске — без копирования. Подпапки тоже просматриваются, новое подхватывается само.</p>
  <div class="line">
    {#if status.canPick && inWindow}
      <button type="button" class="btn" onclick={pick}><Icon name="folder" size={16} /> Выбрать…</button>
    {/if}
    <input class="input" bind:value={path} placeholder={navigator.platform.startsWith('Win') ? 'D:\\Фильмы' : '/Users/имя/Movies'} required />
    <select class="input kind" bind:value={kind} aria-label="Что в папке">
      {#each Object.entries(KINDS) as [k, l] (k)}<option value={k}>{l}</option>{/each}
    </select>
    <button class="btn primary" disabled={busy || !path}><Icon name="plus" size={16} /> Добавить</button>
  </div>
  {#if error}<p class="error">{error}</p>{/if}
</form>

<div class="head">
  <h3>Папки на сайте</h3>
  <span class="muted small">
    {#if status.scan.running}Просматриваем…{:else if status.scan.at}Проверено {ago(status.scan.at)}{/if}
  </span>
  <button class="btn sm" onclick={rescan} disabled={status.scan.running}><Icon name="refresh" size={15} /> Обновить</button>
</div>

{#if libs.length}
  <ul>
    {#each libs as l (l.id)}
      <li class="card">
        <Icon name="folder" size={22} />
        <div class="info">
          <b>{l.path}</b>
          <span class="muted">
            {KINDS[l.kind]} · {count(l.files, 'файл', 'файла', 'файлов')} · {size(l.size)}
            {#if !l.exists}<span class="warn"> · диск не подключён</span>{/if}
          </span>
        </div>
        <button class="btn ghost sm icon danger" aria-label="Убрать" title="Убрать с сайта" onclick={() => remove(l)}><Icon name="trash" size={16} /></button>
      </li>
    {/each}
  </ul>
{:else}
  <p class="empty">Папок пока нет</p>
{/if}

<section class="card pane tips">
  <h3><Icon name="info" size={18} /> Как называть файлы</h3>
  <ul class="names">
    <li><code>Начало (2010)/Inception.2010.1080p.mkv</code> — фильм</li>
    <li><code>Во все тяжкие/Сезон 1/Breaking.Bad.S01E01.mkv</code> — серия</li>
    <li><code>Кухня/2 сезон/Серия 7.mp4</code> — тоже понятно</li>
    <li><code>Фильм.ru.srt</code> рядом с видео — субтитры, <code>poster.jpg</code> — свой постер</li>
  </ul>
  <p class="muted small">Ошиблись с разбором? Исправьте во вкладке «Каталог».</p>
</section>

<style>
  .pane {
    padding: 18px 20px;
    margin-bottom: 18px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
  }
  .add p {
    margin: 6px 0 14px;
    font-size: 14px;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .line .input {
    flex: 1 1 260px;
  }
  .line .kind {
    flex: 0 1 210px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 6px 0 12px;
  }
  .head h3 {
    flex: 1;
  }
  .small {
    font-size: 13px;
  }
  ul {
    list-style: none;
    margin: 0 0 18px;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  li.card {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
  }
  .info {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .info b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .info span {
    font-size: 13px;
  }
  .warn {
    color: var(--warn);
  }
  .empty {
    padding: 24px;
  }
  .names {
    gap: 6px;
    margin: 12px 0;
    font-size: 14px;
  }
  code {
    padding: 1px 6px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.35);
    font-size: 13px;
  }
  .tips p {
    margin: 0;
  }
</style>
