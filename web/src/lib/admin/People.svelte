<script lang="ts">
  import { api, del, errorText, patch, post } from '$lib/api';
  import { ago } from '$lib/fmt';
  import { session } from '$lib/session.svelte';
  import { ask } from '$lib/ui/ask.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Modal from '$lib/ui/Modal.svelte';

  interface U {
    id: number;
    login: string;
    name: string;
    role: 'host' | 'admin' | 'viewer';
    canDownload: boolean;
    disabled: boolean;
    seenAt: number | null;
    hasPassword: boolean;
  }
  interface Inv {
    code: string;
    link: string | null;
    note: string | null;
    usesLeft: number | null;
    expiresAt: number | null;
    used: number;
    active: boolean;
  }

  let users = $state<U[]>([]);
  let invites = $state<Inv[]>([]);
  let error = $state('');
  let copied = $state('');

  let invOpen = $state(false);
  let inv = $state({ note: '', uses: '1', days: '7' });
  let created = $state<{ code: string; link: string | null } | null>(null);

  let userOpen = $state(false);
  let nu = $state({ name: '', login: '', password: '', role: 'viewer' });

  const ROLES = { host: 'Хост', admin: 'Администратор', viewer: 'Зритель' };
  const isHost = $derived(session.me?.user.role === 'host');

  function load() {
    api<U[]>('/api/admin/users').then((u) => (users = u), (e) => (error = errorText(e)));
    api<Inv[]>('/api/admin/invites').then((i) => (invites = i), () => {});
  }
  $effect(load);

  async function copy(text: string) {
    await navigator.clipboard?.writeText(text).catch(() => {});
    copied = text;
    setTimeout(() => (copied = ''), 1500);
  }

  async function createInvite(e: SubmitEvent) {
    e.preventDefault();
    try {
      created = await post('/api/admin/invites', { note: inv.note, uses: Number(inv.uses) || null, days: Number(inv.days) || null });
      load();
    } catch (x) {
      error = errorText(x);
    }
  }

  async function removeInvite(code: string) {
    await del(`/api/admin/invites/${code}`).catch(() => {});
    load();
  }

  async function createUser(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    try {
      await post('/api/admin/users', nu);
      userOpen = false;
      nu = { name: '', login: '', password: '', role: 'viewer' };
      load();
    } catch (x) {
      error = errorText(x);
    }
  }

  async function update(u: U, body: Record<string, unknown>) {
    error = '';
    try {
      await patch(`/api/admin/users/${u.id}`, body);
      load();
    } catch (x) {
      error = errorText(x);
    }
  }

  async function remove(u: U) {
    if (!(await ask(`Удалить ${u.name}? Его прогресс и список тоже удалятся.`, 'Удалить', true))) return;
    await del(`/api/admin/users/${u.id}`).catch((x) => (error = errorText(x)));
    load();
  }

  async function resetPassword(u: U) {
    const pw = Math.random().toString(36).slice(2, 10);
    if (!(await ask(`Новый пароль для ${u.name}: ${pw}\nПередайте его человеку. Сменить?`, 'Сменить'))) return;
    await update(u, { password: pw });
    copy(pw);
  }
</script>

{#if error}<p class="error">{error}</p>{/if}

<section class="card pane">
  <div class="head">
    <h3><Icon name="mail" size={18} /> Приглашения</h3>
    <button
      class="btn primary sm"
      onclick={() => {
        created = null;
        invOpen = true;
      }}><Icon name="plus" size={15} /> Пригласить</button
    >
  </div>
  <p class="muted small">Ссылка-приглашение: человек откроет её, придумает логин и пароль — и сразу смотрит.</p>
  {#if invites.length}
    <ul class="inv">
      {#each invites as i (i.code)}
        <li class:off={!i.active}>
          <code>{i.code}</code>
          <span class="muted">
            {i.note ?? ''}
            {i.usesLeft === null ? 'без ограничений' : `осталось ${i.usesLeft}`}{i.expiresAt ? ` · до ${new Date(i.expiresAt).toLocaleDateString('ru-RU')}` : ''}
            · вошли {i.used}
          </span>
          <span class="grow"></span>
          {#if i.link && i.active}
            <button class="btn sm" onclick={() => copy(i.link!)}><Icon name={copied === i.link ? 'check' : 'link'} size={14} /> Ссылка</button>
          {/if}
          <button class="btn sm icon ghost" aria-label="Удалить" onclick={() => removeInvite(i.code)}><Icon name="trash" size={15} /></button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<section class="card pane">
  <div class="head">
    <h3><Icon name="users" size={18} /> Люди</h3>
    <button class="btn sm" onclick={() => (userOpen = true)}><Icon name="plus" size={15} /> Добавить вручную</button>
  </div>
  <ul class="users">
    {#each users as u (u.id)}
      <li class:off={u.disabled}>
        <span class="ava">{u.name[0]?.toUpperCase()}</span>
        <div class="info">
          <b>{u.name} <span class="role {u.role}">{ROLES[u.role]}</span></b>
          <span class="muted">@{u.login} · {u.disabled ? 'доступ закрыт' : `был ${ago(u.seenAt)}`}</span>
        </div>
        {#if u.role !== 'host'}
          <label class="sw" title="Может скачивать">
            <input type="checkbox" checked={u.canDownload} onchange={(e) => update(u, { canDownload: (e.target as HTMLInputElement).checked })} />
            <Icon name="download" size={15} />
          </label>
          {#if isHost}
            <select class="input role-sel" value={u.role} onchange={(e) => update(u, { role: (e.target as HTMLSelectElement).value })} aria-label="Роль">
              <option value="viewer">Зритель</option>
              <option value="admin">Администратор</option>
            </select>
          {/if}
          <button class="btn sm icon ghost" title="Новый пароль" aria-label="Новый пароль" onclick={() => resetPassword(u)}><Icon name="refresh" size={15} /></button>
          <button class="btn sm icon ghost" title={u.disabled ? 'Открыть доступ' : 'Закрыть доступ'} aria-label="Доступ" onclick={() => update(u, { disabled: !u.disabled })}>
            <Icon name={u.disabled ? 'eyeoff' : 'eye'} size={15} />
          </button>
          <button class="btn sm icon ghost danger" aria-label="Удалить" onclick={() => remove(u)}><Icon name="trash" size={15} /></button>
        {/if}
      </li>
    {/each}
  </ul>
</section>

<Modal bind:open={invOpen} title="Новое приглашение">
  {#if created}
    <div class="created">
      <p>Отправьте ссылку другу:</p>
      {#if created.link}
        <code class="big">{created.link}</code>
        <button class="btn primary" onclick={() => copy(created!.link!)}><Icon name="copy" size={15} /> {copied ? 'Скопировано' : 'Копировать ссылку'}</button>
      {:else}
        <p class="muted">Код: <code>{created.code}</code>. Включите доступ из интернета, чтобы появилась ссылка.</p>
      {/if}
    </div>
  {:else}
    <form class="form" onsubmit={createInvite}>
      <label class="field"><span>Для кого (необязательно)</span><input class="input" bind:value={inv.note} maxlength="80" placeholder="Например: Саша" /></label>
      <div class="two">
        <label class="field">
          <span>Сколько человек</span>
          <select class="input" bind:value={inv.uses}><option value="1">Один</option><option value="5">До 5</option><option value="0">Без ограничений</option></select>
        </label>
        <label class="field">
          <span>Действует</span>
          <select class="input" bind:value={inv.days}><option value="1">1 день</option><option value="7">Неделю</option><option value="30">Месяц</option><option value="0">Всегда</option></select>
        </label>
      </div>
      <button class="btn primary">Создать ссылку</button>
    </form>
  {/if}
</Modal>

<Modal bind:open={userOpen} title="Добавить человека">
  <form class="form" onsubmit={createUser}>
    <label class="field"><span>Имя</span><input class="input" bind:value={nu.name} required /></label>
    <label class="field"><span>Логин</span><input class="input" bind:value={nu.login} required minlength="3" autocomplete="off" /></label>
    <label class="field"><span>Пароль</span><input class="input" bind:value={nu.password} required minlength="6" autocomplete="new-password" /></label>
    {#if isHost}
      <label class="field">
        <span>Роль</span>
        <select class="input" bind:value={nu.role}><option value="viewer">Зритель</option><option value="admin">Администратор — управляет каталогом и людьми</option></select>
      </label>
    {/if}
    <button class="btn primary">Добавить</button>
  </form>
</Modal>

<style>
  .pane {
    padding: 18px 20px;
    margin-bottom: 16px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
  }
  .small {
    font-size: 13px;
    margin: 6px 0 12px;
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
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.2);
    font-size: 13px;
  }
  li.off {
    opacity: 0.5;
  }
  .grow {
    flex: 1;
  }
  code {
    padding: 2px 8px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.35);
    font-size: 13px;
    letter-spacing: 0.05em;
  }
  .users {
    margin-top: 14px;
  }
  .ava {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--surface-3);
    font-weight: 700;
  }
  .info {
    flex: 1;
    min-width: 160px;
    display: grid;
  }
  .info b {
    font-size: 14px;
  }
  .role {
    margin-left: 6px;
    padding: 1px 7px;
    border-radius: 99px;
    font-size: 11px;
    font-weight: 600;
    background: var(--surface-2);
  }
  .role.host {
    background: var(--accent-grad);
  }
  .role.admin {
    background: rgba(138, 99, 255, 0.35);
  }
  .sw {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--muted);
    cursor: pointer;
  }
  .role-sel {
    width: auto;
    height: 34px;
    font-size: 13px;
  }
  .form {
    display: grid;
    gap: 14px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .form .btn {
    justify-self: start;
  }
  .created {
    display: grid;
    gap: 12px;
    justify-items: start;
  }
  .created p {
    margin: 0;
  }
  .big {
    display: block;
    width: 100%;
    padding: 12px;
    font-size: 14px;
    word-break: break-all;
    letter-spacing: 0;
  }
</style>
