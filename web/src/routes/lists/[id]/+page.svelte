<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, del, errorText, patch, post, put } from '$lib/api';
  import { day } from '$lib/library';
  import type { Member, SharedItem, SharedListFull } from '$lib/types';
  import Avatar from '$lib/ui/Avatar.svelte';
  import { ask } from '$lib/ui/dialogs.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Modal from '$lib/ui/Modal.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import Poster from '$lib/discover/Poster.svelte';
  import { extSheet } from '$lib/discover/sheet.svelte';

  // Совместный список: участники, фильмы (с сервиса и из ленты IMDb) по группам, «посмотрели
  // вместе». Меняют все участники.
  const id = $derived(Number(page.params.id));
  let l = $state<SharedListFull | null>(null);
  let error = $state('');
  let inviting = $state(false);
  let friends = $state<Member[]>([]);

  function load() {
    api<SharedListFull>(`/api/lists/${id}`).then((r) => (l = r), (e) => (error = errorText(e)));
  }
  $effect(() => {
    void id;
    void extSheet.version;
    load();
  });

  const key = (i: SharedItem) => (i.ext ? i.ext.id : `t${i.card!.id}`);
  const path = (i: SharedItem) => (i.ext ? `/api/lists/${id}/ext/${i.ext.id}` : `/api/lists/${id}/items/${i.card!.id}`);
  // Группы по порядку, «Без группы» — последней (или единственной, если групп нет).
  // Внутри — сначала то, что ещё не смотрели.
  const sections = $derived.by(() => {
    if (!l) return [];
    const order = (a: SharedItem, b: SharedItem) => Number(!!a.watchedAt) - Number(!!b.watchedAt);
    const out = l.groups.map((g) => ({ id: g.id as number | null, name: g.name, items: l!.items.filter((i) => i.groupId === g.id).sort(order) }));
    const rest = l.items.filter((i) => !l!.groups.some((g) => g.id === i.groupId)).sort(order);
    if (rest.length || !out.length) out.push({ id: null, name: l.groups.length ? 'Без группы' : '', items: rest });
    return out;
  });

  async function watched(i: SharedItem) {
    const before = i.watchedAt;
    i.watchedAt = before ? null : Date.now();
    try {
      await put(`${path(i)}/watched`, { value: !before });
    } catch {
      i.watchedAt = before;
    }
  }

  async function remove(i: SharedItem) {
    if (!l) return;
    l.items = l.items.filter((x) => x !== i);
    await put(path(i), { value: false }).catch(load);
  }

  async function move(i: SharedItem, value: string) {
    const group = value ? Number(value) : null;
    const before = i.groupId;
    i.groupId = group;
    try {
      await put(`/api/lists/${id}/move`, { titleId: i.card?.id ?? null, extId: i.ext?.id ?? null, group });
    } catch (x) {
      i.groupId = before;
      toast(errorText(x));
    }
  }

  let grouping = $state<{ id: number | null; name: string } | null>(null);
  let groupOpen = $state(false);
  const editGroup = (g: { id: number | null; name: string }) => ((grouping = g), (groupOpen = true));
  async function saveGroup(e: SubmitEvent) {
    e.preventDefault();
    const g = grouping;
    if (!g || !g.name.trim()) return;
    try {
      if (g.id) await patch(`/api/lists/${id}/groups/${g.id}`, { name: g.name });
      else await post(`/api/lists/${id}/groups`, { name: g.name });
      groupOpen = false;
      load();
    } catch (x) {
      toast(errorText(x));
    }
  }

  async function dropGroup(g: { id: number | null; name: string }) {
    if (!g.id || !(await ask(`Удалить группу «${g.name}»? Фильмы останутся в списке.`, 'Удалить', true))) return;
    await del(`/api/lists/${id}/groups/${g.id}`).catch((x) => toast(errorText(x)));
    load();
  }

  let renaming = $state(false);
  let newName = $state('');
  async function rename(e: SubmitEvent) {
    e.preventDefault();
    if (!l || !newName.trim()) return;
    try {
      await patch(`/api/lists/${id}`, { name: newName });
      l.name = newName.trim();
      renaming = false;
    } catch (x) {
      toast(errorText(x));
    }
  }

  async function leave() {
    if (!l) return;
    const q = l.mine ? `Удалить список «${l.name}» у всех участников?` : `Выйти из списка «${l.name}»?`;
    if (!(await ask(q, l.mine ? 'Удалить' : 'Выйти', true))) return;
    await del(`/api/lists/${id}`).catch(() => {});
    goto('/lists', { replaceState: true });
  }

  async function openInvite() {
    inviting = true;
    friends = (await api<Member[]>('/api/members').catch(() => [] as Member[])).filter(
      (m) => m.relation === 'friend' && !l?.members.some((x) => x.id === m.id),
    );
  }

  async function invite(m: Member) {
    try {
      await post(`/api/lists/${id}/members`, { id: m.id });
      inviting = false;
      toast(`${m.name} теперь в списке`);
      load();
    } catch (x) {
      toast(errorText(x));
    }
  }
</script>

<div class="page">
  {#if error}
    <div class="empty"><Icon name="info" size={36} /> {error}</div>
  {:else if !l}
    <div class="ph-head skeleton"></div>
  {:else}
    <a class="back muted" href="/lists"><Icon name="back" size={16} /> Все списки</a>
    <div class="head">
      <h1>{l.name}</h1>
      <div class="acts">
        <button class="btn sm ghost" onclick={() => ((newName = l?.name ?? ''), (renaming = true))}><Icon name="edit" size={14} /> Переименовать</button>
        <button class="btn sm ghost" onclick={leave}><Icon name={l.mine ? 'trash' : 'logout'} size={14} /> {l.mine ? 'Удалить' : 'Выйти'}</button>
      </div>
    </div>
    <div class="members">
      {#each l.members as m (m.id)}
        <a class="m" href="/people/{m.id}" title={m.name}><Avatar avatar={m.avatar} photo={m.photo} size={36} /><span>{m.name}</span></a>
      {/each}
      <button class="btn sm" onclick={openInvite}><Icon name="plus" size={14} /> Позвать друга</button>
    </div>

    {#if !l.items.length}
      <div class="empty">
        <Icon name="film" size={40} />
        <p>Список пуст. Откройте фильм здесь или в «Обзоре» и добавьте его в список.</p>
        <a class="btn sm" href="/discover">Открыть обзор</a>
      </div>
    {/if}
    {#each sections as g (g.id ?? 'rest')}
      {#if g.items.length || g.id}
        <section aria-label={g.name || 'Фильмы'}>
          {#if g.name}
            <div class="ghead">
              <h2>{g.name} <span class="faint">{g.items.length}</span></h2>
              {#if g.id}
                <button class="mini" aria-label="Переименовать группу" title="Переименовать" onclick={() => editGroup({ id: g.id, name: g.name })}><Icon name="edit" size={14} /></button>
                <button class="mini" aria-label="Удалить группу" title="Удалить группу" onclick={() => dropGroup(g)}><Icon name="trash" size={14} /></button>
              {/if}
            </div>
          {/if}
          {#if g.items.length}
            <div class="tiles">
              {#each g.items as i (key(i))}
                <div class="item" class:done={!!i.watchedAt}>
                  <Poster card={i.ext} local={i.card ?? null} />
                  <small class="muted who">
                    {#if i.watchedAt}Посмотрели {day(i.watchedAt)}{:else if i.addedBy}Добавил(а) {i.addedBy.name}{:else}&nbsp;{/if}
                  </small>
                  <div class="foot">
                    <button class="mini" class:on={!!i.watchedAt} aria-label={i.watchedAt ? 'Не смотрели' : 'Посмотрели вместе'} title={i.watchedAt ? 'Снять отметку' : 'Посмотрели вместе'} onclick={() => watched(i)}>
                      <Icon name="check" size={15} stroke={2.6} />
                    </button>
                    {#if l.groups.length}
                      <select class="move" aria-label="Группа" value={i.groupId ?? ''} onchange={(e) => move(i, e.currentTarget.value)}>
                        <option value="">Без группы</option>
                        {#each l.groups as x (x.id)}<option value={x.id}>{x.name}</option>{/each}
                      </select>
                    {/if}
                    <button class="mini" aria-label="Убрать из списка" title="Убрать из списка" onclick={() => remove(i)}><Icon name="close" size={15} /></button>
                  </div>
                </div>
              {/each}
            </div>
          {:else}
            <p class="faint none">Пусто — переложите сюда фильмы выбором группы под постером.</p>
          {/if}
        </section>
      {/if}
    {/each}
    <button class="btn sm ghost" onclick={() => editGroup({ id: null, name: '' })}><Icon name="plus" size={14} /> Новая группа</button>
  {/if}
</div>

<Modal bind:open={renaming} title="Название списка">
  <form class="rename" onsubmit={rename}>
    <input class="input" bind:value={newName} maxlength="60" required aria-label="Название" />
    <button class="btn primary">Сохранить</button>
  </form>
</Modal>

<Modal bind:open={groupOpen} title={grouping?.id ? 'Группа' : 'Новая группа'}>
  {#if grouping}
    <form class="rename" onsubmit={saveGroup}>
      <input class="input" bind:value={grouping.name} maxlength="60" required placeholder="Например: Марвел, На выходные" aria-label="Название группы" />
      <button class="btn primary">{grouping.id ? 'Сохранить' : 'Создать'}</button>
    </form>
  {/if}
</Modal>

<Modal bind:open={inviting} title="Позвать друга">
  {#if friends.length}
    <ul class="pick">
      {#each friends as f (f.id)}
        <li><button onclick={() => invite(f)}><Avatar avatar={f.avatar} photo={f.photo} size={32} /> <span>{f.name}</span></button></li>
      {/each}
    </ul>
  {:else}
    <p class="muted">Все друзья уже здесь — или друзей пока нет. Добавьте их в разделе «Люди».</p>
  {/if}
</Modal>

<style>
  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 14px;
    margin-bottom: 8px;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px 16px;
  }
  h1 {
    font-size: clamp(28px, 4vw, 40px);
    font-weight: 800;
    overflow-wrap: anywhere;
    min-width: 0;
  }
  .acts {
    display: flex;
    gap: 6px;
  }
  .members {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 16px 0 28px;
  }
  .m {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px 4px 4px;
    border-radius: 999px;
    background: var(--surface);
    font-size: 14px;
    font-weight: 600;
    max-width: 200px;
  }
  .m span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  h2 {
    font-size: 20px;
  }
  .ghead {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 14px;
  }
  .ghead h2 {
    margin-right: 6px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 150px), 1fr));
    gap: 22px 16px;
  }
  @media (max-width: 640px) {
    .tiles {
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 16px 10px;
    }
  }
  .item.done :global(.art) {
    opacity: 0.55;
  }
  .who {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
  }
  .move {
    flex: 1;
    min-width: 0;
    height: 30px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 12px;
  }
  .none {
    font-size: 14px;
  }
  section {
    margin-bottom: 32px;
  }
  .item {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .foot {
    justify-content: space-between;
  }
  .mini {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex: none;
    border-radius: 50%;
    border: 1px solid var(--line);
    background: var(--surface);
    cursor: pointer;
    transition:
      background 0.15s,
      color 0.15s;
  }
  .mini:hover {
    background: var(--surface-2);
  }
  .mini.on {
    background: #fff;
    color: #000;
  }
  .ph-head {
    height: 120px;
    border-radius: 16px;
  }
  .empty .btn {
    margin-top: 6px;
  }
  .rename {
    display: grid;
    gap: 14px;
  }
  .rename .btn {
    justify-self: end;
  }
  .pick {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .pick button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border: 0;
    border-radius: 10px;
    background: none;
    cursor: pointer;
    text-align: left;
  }
  .pick button:hover {
    background: var(--surface-2);
  }
</style>
