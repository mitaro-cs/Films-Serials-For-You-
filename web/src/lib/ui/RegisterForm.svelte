<script lang="ts">
  import PasswordInput from '$lib/ui/PasswordInput.svelte';
  import { goto } from '$app/navigation';
  import { api, errorText, post } from '$lib/api';
  import { toast } from '$lib/ui/toast.svelte';
  import { loadMe } from '$lib/session.svelte';

  let { invite = '', askInvite = true }: { invite?: string; askInvite?: boolean } = $props();

  let code = $state('');
  let name = $state('');
  let login = $state('');
  let password = $state('');
  let error = $state('');
  let busy = $state(false);
  let email=$state(''),emailEnabled=$state(false);
  $effect(()=>{api<{emailRecovery?:boolean}>('/api/site').then(s=>emailEnabled=!!s.emailRecovery,()=>{});});

  $effect(() => {
    code = invite;
  });

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      await post('/api/auth/register', { invite: code || null, name, login, password });
      if(emailEnabled&&email.trim()) {try{await post('/api/me/email',{email,password});}catch{toast('Аккаунт создан, но письмо не отправлено. Повторите подтверждение в настройках аккаунта.');}}
      await loadMe();
      goto(email.trim()?'/profile?tab=account':'/', { replaceState: true });
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
  <div class="field"><PasswordInput caption="Пароль (от 6 символов)" bind:value={password} autocomplete="new-password" required minlength={6} />
  </div>
  {#if emailEnabled}<label class="field"><span>Почта для восстановления (необязательно)</span><input class="input" type="email" bind:value={email} autocomplete="email"/><small class="muted">После регистрации отправим ссылку для подтверждения.</small></label>{/if}
  {#if error}<p class="error">{error}</p>{/if}
  <button class="btn primary block" disabled={busy}>{busy ? 'Создаём…' : 'Создать аккаунт'}</button>
  <a class="btn ghost block" href="/login">У меня уже есть аккаунт</a>
</form>
