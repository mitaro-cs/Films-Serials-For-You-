<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, del, errorText, patch, post } from '$lib/api';
  import { loadMe, session } from '$lib/session.svelte';
  import type { Profile } from '$lib/types';
  import Avatar from '$lib/ui/Avatar.svelte';
  import { ask } from '$lib/ui/dialogs.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Logo from '$lib/ui/Logo.svelte';
  import Modal from '$lib/ui/Modal.svelte';

  let manage = $state(page.url.searchParams.get('manage') === '1');
  let editing = $state<Profile | null>(null);
  let open = $state(false);
  let form = $state({ name: '', avatar: 0, kids: false, autoplay: true });
  let error = $state('');
  let leaving = $state<number | null>(null);
  let photo = $state<string | null>(null);
  let uploading = $state(false);

  const profiles = $derived(session.me?.profiles ?? []);

  async function choose(p: Profile) {
    if (manage) return edit(p);
    leaving = p.id;
    await post(`/api/profiles/${p.id}/select`).catch(() => {});
    await loadMe();
    const next = page.url.searchParams.get('next');
    goto(next && next.startsWith('/') && !next.startsWith('//') ? next : '/', { replaceState: true });
  }

  function edit(p: Profile | null) {
    editing = p;
    form = p
      ? { name: p.name, avatar: p.avatar, kids: p.kids, autoplay: p.autoplay }
      : { name: '', avatar: (profiles.length * 5) % 12, kids: false, autoplay: true };
    photo = p?.photo ?? null;
    error = '';
    open = true;
  }

  // Своё фото: сервер обрежет до квадрата 256 px.
  async function pick(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!f || !editing) return;
    if (f.size > 15 * 1024 * 1024) {
      error = 'Картинка больше 15 МБ';
      return;
    }
    uploading = true;
    error = '';
    try {
      const r = await api<{ photo: string }>(`/api/profiles/${editing.id}/photo`, { method: 'PUT', body: f });
      photo = r.photo;
      await loadMe();
    } catch (x) {
      error = errorText(x);
    } finally {
      uploading = false;
    }
  }

  async function dropPhoto() {
    if (!editing) return;
    await del(`/api/profiles/${editing.id}/photo`).catch(() => {});
    photo = null;
    await loadMe();
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    try {
      if (editing) await patch(`/api/profiles/${editing.id}`, form);
      else await post('/api/profiles', form);
      open = false;
      await loadMe();
    } catch (x) {
      error = errorText(x);
    }
  }

  async function remove() {
    if (!editing) return;
    if (!(await ask(`Удалить профиль «${editing.name}»? Его история, список и оценки удалятся.`, 'Удалить', true))) return;
    try {
      await del(`/api/profiles/${editing.id}`);
      open = false;
      await loadMe();
    } catch (x) {
      error = errorText(x);
    }
  }
</script>

<div class="who">
  <a class="logo" href="/" aria-label="На главную"><Logo /></a>
  <h1>{manage ? 'Управление профилями' : 'Кто смотрит?'}</h1>
  <ul>
    {#each profiles as p, i (p.id)}
      <li style:--i={i}>
        <button class="p" class:leaving={leaving === p.id} onclick={() => choose(p)}>
          <span class="pic">
            <Avatar avatar={p.avatar} size={132} kids={p.kids} photo={p.photo} />
            {#if manage}<span class="pen"><Icon name="edit" size={30} /></span>{/if}
          </span>
          <span class="name">{p.name}</span>
          {#if p.kids}<span class="kid">Детский</span>{/if}
        </button>
      </li>
    {/each}
    {#if profiles.length < 6}
      <li style:--i={profiles.length}>
        <button class="p add" onclick={() => edit(null)}>
          <span class="pic plus"><Icon name="plus" size={56} stroke={1.5} /></span>
          <span class="name">Добавить профиль</span>
        </button>
      </li>
    {/if}
  </ul>
  <button class="btn ghost manage" onclick={() => (manage = !manage)}>{manage ? 'Готово' : 'Управление профилями'}</button>
</div>

<Modal bind:open title={editing ? 'Изменить профиль' : 'Новый профиль'}>
  <form class="form" onsubmit={save}>
    <div class="head">
      <Avatar avatar={form.avatar} size={84} kids={form.kids} {photo} />
      <label class="field grow"><span>Имя</span><input class="input" bind:value={form.name} maxlength="24" required /></label>
    </div>
    {#if editing}
      <div class="photo-row">
        <label class="btn sm" class:busy={uploading}>
          <Icon name="image" size={15} /> {uploading ? 'Загружаем…' : photo ? 'Другое фото' : 'Своё фото'}
          <input type="file" accept="image/jpeg,image/png,image/webp" onchange={pick} hidden />
        </label>
        {#if photo}<button type="button" class="btn sm ghost" onclick={dropPhoto}>Убрать фото</button>{/if}
      </div>
    {/if}
    <div class="avatars" role="radiogroup" aria-label="Аватар">
      {#each Array(12) as _, i (i)}
        <button type="button" role="radio" aria-checked={form.avatar === i} class:on={form.avatar === i} onclick={() => (form.avatar = i)}>
          <Avatar avatar={i} size={44} />
        </button>
      {/each}
    </div>
    <label class="check">
      <input type="checkbox" bind:checked={form.kids} />
      <span><b>Детский профиль</b><small class="muted">Только мультфильмы и фильмы до 12+</small></span>
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={form.autoplay} />
      <span><b>Превью на главной</b><small class="muted">Фильм начинает играть без звука в большом кадре</small></span>
    </label>
    {#if error}<p class="error">{error}</p>{/if}
    <div class="row">
      {#if editing && profiles.length > 1}<button type="button" class="btn danger" onclick={remove}><Icon name="trash" size={15} /> Удалить</button>{/if}
      <span class="grow"></span>
      <button class="btn primary">Сохранить</button>
    </div>
  </form>
</Modal>

<style>
  .photo-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .photo-row .busy {
    opacity: 0.6;
    pointer-events: none;
  }
  .who {
    min-height: 100dvh;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 28px;
    padding: 80px 16px;
    position: relative;
  }
  .logo {
    position: absolute;
    top: 22px;
    left: var(--gutter);
  }
  h1 {
    font-size: clamp(30px, 5vw, 52px);
    font-weight: 600;
    animation: fade 0.5s ease both;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: clamp(14px, 2.4vw, 30px);
    max-width: 960px;
  }
  li {
    animation: rise 0.5s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(var(--i) * 60ms + 80ms);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(16px) scale(0.96);
    }
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .p {
    display: grid;
    justify-items: center;
    gap: 10px;
    width: clamp(96px, 16vw, 140px);
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    color: var(--muted);
  }
  .pic {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    border-radius: 10px;
    overflow: hidden;
    outline: 3px solid transparent;
    outline-offset: 0;
    transition:
      outline-color 0.15s,
      transform 0.2s;
  }
  .pic :global(svg) {
    width: 100%;
    height: 100%;
  }
  .p:hover .pic,
  .p:focus-visible .pic {
    outline-color: #fff;
  }
  .p:hover .name {
    color: var(--text);
  }
  .p.leaving .pic {
    transform: scale(1.12);
  }
  .plus {
    display: grid;
    place-items: center;
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .pen {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
  }
  .name {
    font-size: 16px;
    line-height: 1.25;
    text-align: center;
    overflow-wrap: anywhere;
    max-width: 100%;
  }
  .kid {
    margin-top: -6px;
    font-size: 11px;
    font-weight: 700;
    color: var(--imdb);
  }
  .manage {
    border: 1px solid var(--faint);
    border-radius: 6px;
    color: var(--muted);
    letter-spacing: 0.1em;
    text-transform: uppercase;
    font-size: 13px;
  }
  .form {
    display: grid;
    gap: 16px;
  }
  .head {
    display: flex;
    align-items: end;
    gap: 16px;
  }
  .grow {
    flex: 1;
  }
  .avatars {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 8px;
  }
  .avatars button {
    padding: 0;
    border: 2px solid transparent;
    border-radius: 9px;
    background: none;
    cursor: pointer;
    justify-self: center;
  }
  .avatars button.on {
    border-color: #fff;
  }
  .check {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    font-size: 14px;
  }
  .check input {
    margin-top: 3px;
  }
  .check span {
    display: grid;
  }
  .check small {
    font-size: 12px;
  }
  .row {
    display: flex;
    gap: 10px;
  }
</style>
