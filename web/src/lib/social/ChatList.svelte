<script lang="ts">
  import { api, errorText } from '$lib/api';
  import { ago } from '$lib/fmt';
  import type { Person } from '$lib/types';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';

  // Собеседники: все друзья, сверху - с последними сообщениями. Обновляется раз в 15 с.
  let { current = null }: { current?: number | null } = $props();

  interface Item {
    person: Person;
    last: string | null;
    at: number | null;
    unread: number;
  }
  let items = $state<Item[] | null>(null);
  let error = $state('');

  $effect(() => {
    const controller = new AbortController();
    let pending = false;
    const load = async () => {
      if (pending) return;
      pending = true;
      try {
        const r = await api<{ items: Item[] }>('/api/chats', { signal: controller.signal });
        if (!controller.signal.aborted) { items = r.items; error = ''; }
      } catch (e) {
        if (!controller.signal.aborted) error = errorText(e);
      } finally { pending = false; }
    };
    load();
    const t = setInterval(() => document.visibilityState === 'visible' && load(), 15_000);
    return () => { clearInterval(t); controller.abort(); };
  });

  // Открыли переписку - у неё больше нет непрочитанных.
  $effect(() => {
    const c = current;
    if (c && items) for (const i of items) if (i.person.id === c) i.unread = 0;
  });
</script>

<nav class="list" aria-label="Переписки">
  {#if error}
    <p class="muted pad">{error}</p>
  {:else if items === null}
    {#each Array(4) as _, i (i)}<div class="row sk skeleton"></div>{/each}
  {:else if items.length === 0}
    <div class="pad none">
      <Icon name="users" size={32} />
      <p>Переписка - с друзьями. <a href="/people">Найдите друзей в «Людях»</a>.</p>
    </div>
  {:else}
    {#each items as i (i.person.id)}
      <a class="row" class:on={current === i.person.id} href="/chats/{i.person.id}" aria-current={current === i.person.id ? 'page' : undefined}>
        <Avatar avatar={i.person.avatar} photo={i.person.photo} size={44} />
        <span class="txt">
          <b>{i.person.name}</b>
          <small class:unread={i.unread}>{i.last ?? 'Напишите первым'}</small>
        </span>
        <span class="side">
          {#if i.at}<small class="faint">{ago(i.at)}</small>{/if}
          {#if i.unread}<i class="num">{i.unread}</i>{/if}
        </span>
      </a>
    {/each}
  {/if}
</nav>

<style>
  .list {
    display: grid;
    gap: 4px;
    align-content: start;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 64px;
    padding: 10px 12px;
    border-radius: 14px;
    transition: background 0.15s;
  }
  .row:hover,
  .row.on {
    background: #232325;
  }
  .sk {
    height: 64px;
  }
  .txt {
    display: grid;
    flex: 1;
    min-width: 0;
  }
  .txt b,
  .txt small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .txt small {
    font-size: 12.5px;
    color: var(--muted);
  }
  .txt small.unread {
    color: var(--text);
    font-weight: 600;
  }
  .side {
    display: grid;
    justify-items: end;
    gap: 4px;
    flex: none;
  }
  .side small {
    font-size: 11px;
  }
  .num {
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 99px;
    background: #ff5c7a;
    color: #fff;
    font-size: 11px;
    font-style: normal;
    font-weight: 800;
    line-height: 20px;
    text-align: center;
  }
  .pad {
    padding: 16px;
  }
  .none {
    display: grid;
    justify-items: center;
    gap: 8px;
    text-align: center;
    color: var(--muted);
  }
  .none p {
    margin: 0;
  }
  .none a {
    color: var(--text);
    text-decoration: underline;
  }
</style>
