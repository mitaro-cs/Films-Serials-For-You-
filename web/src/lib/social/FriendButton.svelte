<script lang="ts">
  import { del, post } from '$lib/api';
  import type { Relation } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import { refreshBadges } from './badges.svelte';

  // Кнопка дружбы: добавить → «заявка отправлена»; просит меня → принять / отклонить; друг → написать.
  let { id, relation = $bindable(), small = false }: { id: number; relation: Relation; small?: boolean } = $props();
  let busy = $state(false);

  async function go(f: () => Promise<{ relation: Relation }>) {
    busy = true;
    try {
      relation = (await f()).relation;
      refreshBadges();
    } catch {
      /* попробуют ещё раз */
    } finally {
      busy = false;
    }
  }
  const add = () => go(() => post(`/api/friends/${id}`));
  const drop = () => go(() => del(`/api/friends/${id}`));
</script>

<span class="fb">
  {#if relation === 'friend'}
    <a class="btn primary" class:sm={small} href="/chats/{id}"><Icon name="mail" size={15} /> Написать</a>
  {:else if relation === 'incoming'}
    <button class="btn primary" class:sm={small} disabled={busy} onclick={add}><Icon name="check" size={15} /> Принять</button>
    <button class="btn ghost" class:sm={small} disabled={busy} onclick={drop}>Отклонить</button>
  {:else if relation === 'outgoing'}
    <button class="btn" class:sm={small} disabled={busy} onclick={drop} title="Отменить заявку"><Icon name="clock" size={15} /> Заявка отправлена</button>
  {:else}
    <button class="btn" class:sm={small} disabled={busy} onclick={add}><Icon name="plus" size={15} /> В друзья</button>
  {/if}
</span>

<style>
  .fb {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 6px;
  }
</style>
