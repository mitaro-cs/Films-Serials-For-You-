<script lang="ts">
  import PasswordInput from '$lib/ui/PasswordInput.svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, ApiError, errorText, post } from '$lib/api';
  import { loadMe } from '$lib/session.svelte';
  import type { Site } from '$lib/types';
  import AuthShell from '$lib/ui/AuthShell.svelte';

  let login = $state('');
  let password = $state('');
  let error = $state('');
  let busy = $state(false);
  // Вход в два шага: пароль верный - сервер просит код из приложения.
  let code = $state('');
  let needCode = $state(false);
  let site = $state<Site | null>(null);

  $effect(() => {
    api<Site>('/api/site').then((s) => (site = s), () => {});
  });

  function safeNext(): string {
    const n = page.url.searchParams.get('next') ?? '/';
    return n.startsWith('/') && !n.startsWith('//') ? n : '/';
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      await post('/api/auth/login', { login, password, code: needCode ? code : undefined });
      await loadMe();
      goto(safeNext(), { replace: true });
    } catch (err) {
      if (err instanceof ApiError && err.code === 'totp_required') {
        needCode = true;
        return;
      }
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<AuthShell title="Вход" subtitle={site ? `Онлайн-кинотеатр «${site.name}»` : undefined}>
  <form onsubmit={submit}>
    <label class="field"><span>Логин</span><input class="input" bind:value={login} autocomplete="username" required /></label>
    <div class="field"><PasswordInput caption="Пароль" bind:value={password} autocomplete="current-password" required />
    </div>
    {#if needCode}
      <label class="field">
        <span>Код из приложения</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input code" bind:value={code} inputmode="numeric" autocomplete="one-time-code" maxlength="7" placeholder="000 000" autofocus required />
      </label>
      <p class="hint">Откройте приложение-аутентификатор (Google Authenticator, Яндекс Ключ) и введите 6 цифр для Nova.</p>
    {:else}
      <a class="forgot" href="/forgot-password">Забыли пароль?</a>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
    <button class="btn primary block" disabled={busy}>{busy ? 'Входим…' : 'Войти'}</button>
    {#if site && site.registration !== 'closed'}
      <a class="btn ghost block" href="/register">{site.registration === 'open' ? 'Создать аккаунт' : 'У меня есть приглашение'}</a>
    {/if}
  </form>
</AuthShell>

<style>.code{font-size:22px;letter-spacing:.3em;text-align:center}.hint{margin:-4px 0 4px;font-size:13px;color:#aaa}.forgot{justify-self:end;font-size:13px;color:#aaa;text-decoration:underline;padding:5px 0}.forgot:hover{color:#fff}</style>
