<script lang="ts">
  import { api, put } from '$lib/api';
  import Icon from '$lib/ui/Icon.svelte';
  import Modal from '$lib/ui/Modal.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import NewList from './NewList.svelte';

  // Со страницы фильма: в какие совместные списки положить (галочки), или новый список.
  let { open = $bindable(false), titleId }: { open: boolean; titleId: number } = $props();

  let lists = $state<{ id: number; name: string; has: boolean }[] | null>(null);
  let creating = $state(false);

  function load() {
    api<{ id: number; name: string; has: boolean }[]>(`/api/titles/${titleId}/lists`).then(
      (r) => (lists = r),
      () => (lists = []),
    );
  }
  $effect(() => {
    if (open) load();
  });

  async function toggle(l: { id: number; name: string; has: boolean }) {
    l.has = !l.has;
    try {
      await put(`/api/lists/${l.id}/items/${titleId}`, { value: l.has });
      toast(l.has ? `Добавили в «${l.name}»` : `Убрали из «${l.name}»`);
    } catch {
      l.has = !l.has;
    }
  }

  async function created(id: number) {
    await put(`/api/lists/${id}/items/${titleId}`, { value: true }).catch(() => {});
    open = true;
    load();
  }
</script>

<Modal bind:open title="Совместные списки">
  <div class="box">
    {#if lists === null}
      <div class="skeleton ph"></div>
    {:else if lists.length === 0}
      <p class="muted">Списков пока нет. Создайте — и смотрите вместе с друзьями.</p>
    {:else}
      <ul>
        {#each lists as l (l.id)}
          <li>
            <button class:on={l.has} role="menuitemcheckbox" aria-checked={l.has} onclick={() => toggle(l)}>
              <span class="tick"><Icon name="check" size={15} stroke={3} /></span>
              <span class="name">{l.name}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
    <div class="row">
      <a class="btn ghost sm" href="/lists" onclick={() => (open = false)}>Все списки</a>
      <button class="btn sm" onclick={() => ((open = false), (creating = true))}><Icon name="plus" size={15} /> Новый список</button>
    </div>
  </div>
</Modal>
<NewList bind:open={creating} oncreated={created} />

<style>
  .box {
    display: grid;
    gap: 14px;
  }
  .ph {
    height: 96px;
    border-radius: 12px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  li button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border: 0;
    border-radius: 10px;
    background: var(--surface);
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }
  li button:hover {
    background: var(--surface-2);
  }
  .tick {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    flex: none;
    border-radius: 6px;
    border: 1.5px solid var(--line-strong);
    color: transparent;
    transition:
      background 0.15s,
      color 0.15s;
  }
  .on .tick {
    background: #fff;
    border-color: #fff;
    color: #000;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
</style>
