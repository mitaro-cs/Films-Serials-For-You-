<script lang="ts">
  import { goto } from '$app/navigation';
  import { errorText, patch, post, put } from '$lib/api';
  import { loadMe, session } from '$lib/session.svelte';
  import Icon from '$lib/ui/Icon.svelte';

  let name = $state('');
  let old = $state('');
  let fresh = $state('');
  let msg = $state('');
  let err = $state('');

  $effect(() => {
    name = session.me?.user.name ?? '';
  });

  async function saveName(e: SubmitEvent) {
    e.preventDefault();
    err = msg = '';
    try {
      await patch('/api/me', { name });
      await loadMe();
      msg = 'Имя сохранено';
    } catch (x) {
      err = errorText(x);
    }
  }

  async function savePassword(e: SubmitEvent) {
    e.preventDefault();
    err = msg = '';
    try {
      await put('/api/me/password', { old: old || null, new: fresh });
      old = fresh = '';
      await loadMe();
      msg = session.me?.window
        ? `Пароль задан. С телефона входите с логином «${session.me.user.login}»`
        : 'Пароль изменён';
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
  <h1 class="section-title" style="font-size: 30px"><Icon name="user" size={26} /> Профиль</h1>
  {#if session.me}
    <form class="card pane" onsubmit={saveName}>
      <h2>Имя</h2>
      <label class="field"><span>Как вас видят другие</span><input class="input" bind:value={name} maxlength="60" /></label>
      <p class="muted small">Логин: <b>{session.me.user.login}</b></p>
      <button class="btn">Сохранить</button>
    </form>
    <form class="card pane" onsubmit={savePassword}>
      <h2>{session.me.hasPassword ? 'Сменить пароль' : 'Пароль для входа с других устройств'}</h2>
      {#if !session.me.hasPassword}
        <p class="muted small">Сейчас вы входите только из приложения на этом компьютере. Задайте пароль, чтобы управлять сайтом с телефона.</p>
      {:else}
        <label class="field"><span>Текущий пароль</span><input class="input" type="password" bind:value={old} autocomplete="current-password" /></label>
      {/if}
      <label class="field"><span>Новый пароль</span><input class="input" type="password" bind:value={fresh} minlength="6" autocomplete="new-password" required /></label>
      <button class="btn">Сохранить пароль</button>
    </form>
    {#if msg}<p class="ok">{msg}</p>{/if}
    {#if err}<p class="error">{err}</p>{/if}
    {#if !session.me.window}
      <button class="btn danger" onclick={logout}><Icon name="logout" size={16} /> Выйти</button>
    {/if}
  {/if}
</div>

<style>
  .narrow {
    max-width: 640px;
    margin: 0 auto;
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
  .small {
    font-size: 13px;
    margin: 0;
  }
  .ok {
    color: var(--ok);
  }
</style>
