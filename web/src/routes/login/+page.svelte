<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, errorText, post } from '$lib/api';
  import { loadMe } from '$lib/session.svelte';
  import type { Site } from '$lib/types';
  import AuthShell from '$lib/ui/AuthShell.svelte';

  let login = $state('');
  let password = $state('');
  let error = $state('');
  let busy = $state(false);
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
      await post('/api/auth/login', { login, password });
      await loadMe();
      goto(safeNext(), { replaceState: true });
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<AuthShell title="Вход" subtitle={site ? `Онлайн-кинотеатр «${site.name}»` : undefined}>
  <form onsubmit={submit}>
    <label class="field"><span>Логин</span><input class="input" bind:value={login} autocomplete="username" required /></label>
    <label class="field">
      <span>Пароль</span><input class="input" type="password" bind:value={password} autocomplete="current-password" required />
    </label>
    {#if error}<p class="error">{error}</p>{/if}
    <button class="btn primary block" disabled={busy}>{busy ? 'Входим…' : 'Войти'}</button>
    {#if site && site.registration !== 'closed'}
      <a class="btn ghost block" href="/register">{site.registration === 'open' ? 'Создать аккаунт' : 'У меня есть приглашение'}</a>
    {/if}
  </form>
</AuthShell>
