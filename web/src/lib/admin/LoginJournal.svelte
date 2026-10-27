<script lang="ts">
  import { api } from '$lib/api';

  // Журнал входов по паролю: кто и с какого браузера (адреса не храним). Свёрнут по умолчанию.
  type Item = { login: string; name: string | null; ok: boolean; agent: string | null; at: number };
  let items = $state<Item[] | null>(null);
  let open = $state(false);

  $effect(() => {
    if (open && !items) api<{ items: Item[] }>('/api/admin/logins').then((r) => (items = r.items), () => (items = []));
  });

  const when = (ms: number) => new Date(ms).toLocaleString('ru-RU', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });
  function device(agent: string | null): string {
    if (!agent) return '';
    const os = /iPhone|iPad/.test(agent) ? 'iPhone/iPad' : /Android/.test(agent) ? 'Android' : /Windows/.test(agent) ? 'Windows' : /Mac OS/.test(agent) ? 'Mac' : /Linux/.test(agent) ? 'Linux' : '';
    return os;
  }
  const failed = $derived(items?.filter((i) => !i.ok).length ?? 0);
</script>

<details bind:open>
  <summary class="muted">Журнал входов</summary>
  {#if items === null}
    <p class="muted">Загружаем…</p>
  {:else if !items.length}
    <p class="muted">Входов по паролю ещё не было.</p>
  {:else}
    {#if failed}<p class="warn">Неудачных попыток: {failed} - если их много подряд, кто-то подбирает пароль (вход сам закрывается на 10 минут).</p>{/if}
    <ul>
      {#each items as i, n (n)}
        <li class:bad={!i.ok}>
          <span>{i.name ?? i.login}{#if device(i.agent)}<small> - {device(i.agent)}</small>{/if}</span>
          <small>{i.ok ? '' : 'неудачно - '}{when(i.at)}</small>
        </li>
      {/each}
    </ul>
  {/if}
</details>

<style>
  ul {
    list-style: none;
    margin: 10px 0 0;
    padding: 0;
    display: grid;
    gap: 6px;
    max-height: 320px;
    overflow-y: auto;
  }
  li {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 13.5px;
  }
  small {
    color: var(--faint);
  }
  .bad small {
    color: #ff8a80;
  }
  .warn {
    margin: 10px 0 0;
    font-size: 13px;
    color: #ffcc80;
  }
</style>
