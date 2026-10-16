<script lang="ts">
  import { goto } from '$app/navigation';
  import { errorText, post } from '$lib/api';
  import { loadMe } from '$lib/session.svelte';

  let { invite = '', askInvite = true }: { invite?: string; askInvite?: boolean } = $props();

  let code = $state('');
  let name = $state('');
  let login = $state('');
  let password = $state('');
  let error = $state('');
  let busy = $state(false);

  $effect(() => {
    code = invite;
  });

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      await post('/api/auth/register', { invite: code || null, name, login, password });
      await loadMe();
      goto('/', { replaceState: true });
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<form onsubmit={submit}>
  {#if askInvite}
    <label class="field">
      <span>Код приглашения</span>
      <input class="input" bind:value={code} autocapitalize="characters" autocomplete="off" required />
    </label>
  {/if}
  <label class="field"><span>Как вас зовут</span><input class="input" bind:value={name} autocomplete="name" required maxlength="60" /></label>
  <label class="field">
    <span>Логин (латиница, цифры)</span><input class="input" bind:value={login} autocomplete="username" required minlength="3" maxlength="32" />
  </label>
  <label class="field">
    <span>Пароль (от 6 символов)</span>
    <input class="input" type="password" bind:value={password} autocomplete="new-password" required minlength="6" />
  </label>
  {#if error}<p class="error">{error}</p>{/if}
  <button class="btn primary block" disabled={busy}>{busy ? 'Создаём…' : 'Создать аккаунт'}</button>
  <a class="btn ghost block" href="/login">У меня уже есть аккаунт</a>
</form>
