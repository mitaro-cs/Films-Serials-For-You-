<script lang="ts">
  import { api, errorText, post } from '$lib/api';
  import type { Person } from '$lib/types';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Modal from '$lib/ui/Modal.svelte';

  // «Обсудить с другом»: выбрать друга — фильм уходит ему в переписку карточкой.
  let { open = $bindable(false), titleId, title }: { open: boolean; titleId: number; title: string } = $props();

  let friends = $state<{ person: Person }[] | null>(null);
  let note = $state('');
  let sent = $state<number[]>([]);
  let error = $state('');

  $effect(() => {
    if (!open) return;
    sent = [];
    error = '';
    api<{ items: { person: Person }[] }>('/api/chats').then((r) => (friends = r.items), (e) => (error = errorText(e)));
  });

  async function send(p: Person) {
    try {
      await post(`/api/chats/${p.id}`, { titleId, body: note });
      sent = [...sent, p.id];
    } catch (x) {
      error = errorText(x);
    }
  }
</script>

<Modal bind:open title="Обсудить «{title}»">
  <div class="share">
    <input class="input" bind:value={note} maxlength="2000" placeholder="Пара слов к фильму (можно без них)" />
    {#if error}<p class="error">{error}</p>{/if}
    {#if friends === null}
      <p class="muted">Загружаем друзей…</p>
    {:else if friends.length === 0}
      <p class="muted">Друзей пока нет. <a href="/people" onclick={() => (open = false)}>Найдите их в «Людях»</a> — переписка только с друзьями.</p>
    {:else}
      <ul>
        {#each friends as f (f.person.id)}
          <li>
            <Avatar avatar={f.person.avatar} photo={f.person.photo} size={36} />
            <span class="name">{f.person.name}</span>
            {#if sent.includes(f.person.id)}
              <a class="btn sm" href="/chats/{f.person.id}"><Icon name="check" size={14} /> В переписку</a>
            {:else}
              <button class="btn sm primary" onclick={() => send(f.person)}>Отправить</button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</Modal>

<style>
  .share {
    display: grid;
    gap: 14px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px;
    border-radius: 12px;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  p {
    margin: 0;
  }
  a:not(.btn) {
    text-decoration: underline;
  }
</style>
