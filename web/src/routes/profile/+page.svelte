<script lang="ts">
  import { goto } from '$app/navigation';
  import { api, del, errorText, patch, post, put } from '$lib/api';
  import { loadMe, session } from '$lib/session.svelte';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  // Мой аккаунт: у каждого один — имя, фото или рисованный аватар, пароль.
  const me = $derived(session.me);
  const profile = $derived(me?.user.profile);

  let name = $state('');
  let old = $state('');
  let fresh = $state('');
  let err = $state('');
  let uploading = $state(false);

  $effect(() => {
    name = session.me?.user.name ?? '';
  });

  async function saveName(e: SubmitEvent) {
    e.preventDefault();
    err = '';
    try {
      await patch('/api/me', { name });
      await loadMe();
      toast('Имя сохранено');
    } catch (x) {
      err = errorText(x);
    }
  }

  async function setAvatar(i: number) {
    await patch('/api/me', { avatar: i }).catch(() => {});
    await loadMe();
  }

  // Своё фото: сервер обрежет до квадрата 256 px.
  async function pick(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const f = input.files?.[0];
    input.value = '';
    if (!f || !profile) return;
    if (f.size > 15 * 1024 * 1024) {
      err = 'Картинка больше 15 МБ';
      return;
    }
    uploading = true;
    err = '';
    try {
      await api(`/api/profiles/${profile.id}/photo`, { method: 'PUT', body: f });
      await loadMe();
    } catch (x) {
      err = errorText(x);
    } finally {
      uploading = false;
    }
  }

  async function dropPhoto() {
    if (!profile) return;
    await del(`/api/profiles/${profile.id}/photo`).catch(() => {});
    await loadMe();
  }

  async function savePassword(e: SubmitEvent) {
    e.preventDefault();
    err = '';
    try {
      await put('/api/me/password', { old: old || null, new: fresh });
      old = fresh = '';
      await loadMe();
      toast(session.me?.window ? `Пароль задан. С телефона входите с логином «${session.me.user.login}»` : 'Пароль изменён');
    } catch (x) {
      err = errorText(x);
    }
  }

  async function logout() {
    await post('/api/auth/logout').catch(() => {});
    session.me = null;
    goto('/login');
  }
</script>

<div class="page narrow">
  {#if me && profile}
    <section class="hero card">
      <div class="pic">
        <Avatar avatar={profile.avatar} photo={profile.photo} size={96} />
      </div>
      <div class="who">
        <h1>{me.user.name}</h1>
        <p class="muted">@{me.user.login}</p>
        <div class="row">
          <label class="btn sm" class:busy={uploading}>
            <Icon name="image" size={15} />
            {uploading ? 'Загружаем…' : profile.photo ? 'Другое фото' : 'Загрузить фото'}
            <input type="file" accept="image/jpeg,image/png,image/webp" onchange={pick} hidden />
          </label>
          {#if profile.photo}<button class="btn sm ghost" onclick={dropPhoto}>Убрать фото</button>{/if}
          <a class="btn sm ghost" href="/people/{profile.id}"><Icon name="user" size={15} /> Моя страница</a>
        </div>
      </div>
    </section>

    {#if !profile.photo}
      <section class="card pane">
        <h2>Аватар</h2>
        <div class="avatars" role="radiogroup" aria-label="Аватар">
          {#each Array(12) as _, i (i)}
            <button type="button" role="radio" aria-checked={profile.avatar === i} aria-label="Аватар {i + 1}" class:on={profile.avatar === i} onclick={() => setAvatar(i)}>
              <Avatar avatar={i} size={44} />
            </button>
          {/each}
        </div>
      </section>
    {/if}

    <form class="card pane" onsubmit={saveName}>
      <h2>Имя</h2>
      <label class="field"><span>Как вас видят другие</span><input class="input" bind:value={name} maxlength="60" /></label>
      <button class="btn">Сохранить</button>
    </form>

    <form class="card pane" onsubmit={savePassword}>
      <h2>{me.hasPassword ? 'Сменить пароль' : 'Пароль для входа с других устройств'}</h2>
      {#if !me.hasPassword}
        <p class="muted small">Сейчас вы входите только из приложения на этом компьютере. Задайте пароль, чтобы управлять сайтом с телефона.</p>
      {:else}
        <label class="field"><span>Текущий пароль</span><input class="input" type="password" bind:value={old} autocomplete="current-password" /></label>
      {/if}
      <label class="field"><span>Новый пароль</span><input class="input" type="password" bind:value={fresh} minlength="6" autocomplete="new-password" required /></label>
      <button class="btn">Сохранить пароль</button>
    </form>
    {#if err}<p class="error">{err}</p>{/if}
    {#if !me.window}
      <button class="btn danger" onclick={logout}><Icon name="logout" size={16} /> Выйти</button>
    {/if}
  {/if}
</div>

<style>
  .narrow {
    max-width: 640px;
    margin: 0 auto;
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 22px;
    margin-bottom: 18px;
  }
  .pic {
    flex: none;
    border-radius: 50%;
    overflow: hidden;
  }
  .who {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  h1 {
    font-size: 26px;
    font-weight: 800;
    overflow-wrap: anywhere;
  }
  .who p {
    margin: 0;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }
  .busy {
    opacity: 0.6;
    pointer-events: none;
  }
  .pane {
    display: grid;
    gap: 14px;
    padding: 22px;
    margin-bottom: 18px;
  }
  .pane h2 {
    font-size: 17px;
  }
  .pane .btn {
    justify-self: start;
  }
  .avatars {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(48px, 1fr));
    gap: 10px;
  }
  .avatars button {
    display: grid;
    place-items: center;
    padding: 2px;
    border: 2px solid transparent;
    border-radius: 50%;
    background: none;
    cursor: pointer;
    transition: transform 0.15s;
  }
  .avatars button:hover {
    transform: scale(1.08);
  }
  .avatars .on {
    border-color: var(--text);
  }
  .small {
    font-size: 13px;
    margin: 0;
  }
  @media (max-width: 480px) {
    .hero {
      flex-direction: column;
      text-align: center;
    }
    .row {
      justify-content: center;
    }
  }
</style>
