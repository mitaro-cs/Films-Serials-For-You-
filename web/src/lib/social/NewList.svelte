<script lang="ts">
  import { api, errorText, post } from '$lib/api';
  import type { Member } from '$lib/types';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Modal from '$lib/ui/Modal.svelte';

  // Новый совместный список: название и друзья, с кем смотреть.
  let { open = $bindable(false), oncreated }: { open: boolean; oncreated: (id: number) => void } = $props();

  let name = $state('');
  let picked = $state<number[]>([]);
  let friends = $state<Member[] | null>(null);
  let error = $state('');

  $effect(() => {
    if (!open) return;
    name = '';
    picked = [];
    error = '';
    api<Member[]>('/api/members').then(
      (r) => (friends = r.filter((m) => m.relation === 'friend')),
      () => (friends = []),
    );
  });

  function toggle(id: number) {
    picked = picked.includes(id) ? picked.filter((x) => x !== id) : [...picked, id];
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    try {
      const r = await post<{ id: number }>('/api/lists', { name, members: picked });
      open = false;
      oncreated(r.id);
    } catch (x) {
      error = errorText(x);
    }
  }
</script>

<Modal bind:open title="Новый совместный список">
  <form class="form" onsubmit={save}>
    <label class="field"><span>Название</span><input class="input" bind:value={name} maxlength="60" placeholder="Например, «Кино по пятницам»" required /></label>
    <div class="field">
      <span>С кем смотрим</span>
      {#if friends === null}
        <div class="skeleton ph"></div>
      {:else if friends.length === 0}
        <p class="muted small">Друзей пока нет — добавьте их в разделе «Люди», а пока список будет только вашим.</p>
      {:else}
        <ul class="who">
          {#each friends as f (f.id)}
            <li>
              <label class:on={picked.includes(f.id)}>
                <input type="checkbox" checked={picked.includes(f.id)} onchange={() => toggle(f.id)} />
                <Avatar avatar={f.avatar} photo={f.photo} size={32} />
                <span>{f.name}</span>
              </label>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
    {#if error}<p class="error">{error}</p>{/if}
    <button class="btn primary">Создать</button>
  </form>
</Modal>

<style>
  .form {
    display: grid;
    gap: 16px;
  }
  .form .btn {
    justify-self: end;
  }
  .ph {
    height: 44px;
    border-radius: 10px;
  }
  .small {
    font-size: 13px;
    margin: 0;
  }
  .who {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
    max-height: 40vh;
    overflow-y: auto;
  }
  .who label {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: 10px;
    cursor: pointer;
    min-width: 0;
  }
  .who label:hover,
  .who label.on {
    background: var(--surface-2);
  }
  .who span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who input {
    accent-color: #fff;
    width: 18px;
    height: 18px;
    flex: none;
  }
</style>
