<script lang="ts">
  import { errorText, post } from '$lib/api';
  import { ago } from '$lib/fmt';
  import { session } from '$lib/session.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import type { Status, UpdateInfo } from './types';

  // Версия и обновление с GitHub прямо из приложения: проверить, скачать, поставить.
  let { status, reload }: { status: Status; reload: () => void } = $props();

  let error = $state('');
  const u = $derived(status.update);
  const busy = $derived(!!u && ['checking', 'downloading', 'installing'].includes(u.state));
  const host = $derived(session.me?.user.role === 'host');

  // Пока идёт проверка или скачивание — следим за ходом раз в секунду.
  $effect(() => {
    if (!busy) return;
    const t = setInterval(reload, 1000);
    return () => clearInterval(t);
  });

  async function run(what: 'check' | 'install') {
    error = '';
    try {
      const r = await post<{ update: UpdateInfo }>(`/api/admin/update/${what}`);
      if (status.update) status.update = r.update;
      setTimeout(reload, 400);
    } catch (x) {
      error = errorText(x);
    }
  }

  const TEXT: Record<UpdateInfo['state'], string> = {
    idle: 'Обновления проверяются сами — через 20 секунд после запуска и раз в 6 часов.',
    checking: 'Проверяем обновления на GitHub…',
    latest: 'У вас последняя версия.',
    available: 'Доступна новая версия.',
    downloading: 'Скачиваем обновление…',
    installing: 'Устанавливаем. Приложение перезапустится само, сайт вернётся примерно через минуту.',
    error: '',
  };
</script>

<section class="card pane">
  <div class="ver">
    <img src="/logo.svg" alt="" width="64" height="33" />
    <div>
      <b>Nova</b>
      <span class="muted">Версия {status.version}</span>
    </div>
  </div>

  {#if !u}
    <p class="muted">
      Сервер запущен без приложения — обновите его вручную:
      <a href="https://github.com/mitaro-cs/Films-Serials-For-You-/releases/latest" target="_blank" rel="noreferrer">последний выпуск на GitHub</a>.
    </p>
  {:else}
    <div class="state" class:ok={u.state === 'latest'} class:new={u.state === 'available'} class:bad={u.state === 'error'}>
      <Icon name={u.state === 'latest' ? 'check' : u.state === 'error' ? 'info' : u.state === 'available' ? 'sparkle' : 'refresh'} size={18} />
      <span>
        {u.state === 'error' ? u.error : TEXT[u.state]}
        {#if u.state === 'available' && u.latest}<b>{u.latest}</b>{/if}
        {#if u.checkedAt && !busy}<small class="faint">Проверено {ago(u.checkedAt)}</small>{/if}
      </span>
    </div>
    {#if u.state === 'downloading' || u.state === 'installing'}
      <div class="bar" role="progressbar" aria-valuenow={Math.round((u.progress ?? 0) * 100)} aria-valuemin={0} aria-valuemax={100}>
        <i style:transform="scaleX({u.progress ?? 0})"></i>
      </div>
    {/if}
    <div class="row">
      {#if u.state === 'available'}
        <button class="btn primary" disabled={!host} onclick={() => run('install')}><Icon name="download" size={16} /> Обновить до {u.latest}</button>
      {/if}
      <button class="btn" disabled={busy || !host} onclick={() => run('check')}><Icon name="refresh" size={16} /> Проверить обновления</button>
    </div>
    {#if !host}<p class="faint small">Обновляет только хост — владелец компьютера с приложением.</p>{/if}
  {/if}
  {#if error}<p class="error">{error}</p>{/if}
  <a class="faint small" href="https://github.com/mitaro-cs/Films-Serials-For-You-/releases" target="_blank" rel="noreferrer">Что нового — все выпуски на GitHub</a>
</section>

<style>
  .pane {
    display: grid;
    gap: 16px;
    padding: 22px;
  }
  .ver {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .ver div {
    display: grid;
  }
  .ver b {
    font-size: 18px;
  }
  .state {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--surface);
  }
  .state span {
    display: grid;
    gap: 2px;
  }
  .state.ok {
    color: var(--ok);
  }
  .state.new {
    background: rgba(255, 255, 255, 0.1);
  }
  .state.bad {
    color: var(--danger);
  }
  .bar {
    height: 6px;
    border-radius: 6px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: #fff;
    transform-origin: left;
    transition: transform 0.4s;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .small {
    font-size: 13px;
    margin: 0;
  }
</style>
