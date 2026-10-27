<script lang="ts">
  import { page } from '$app/state';
  import { api } from '$lib/api';
  import AuthShell from '$lib/ui/AuthShell.svelte';
  import RegisterForm from '$lib/ui/RegisterForm.svelte';

  const code = $derived(page.params.code ?? '');
  let state = $state<{ valid: boolean; site: string } | null>(null);

  $effect(() => {
    api<{ valid: boolean; site: string }>(`/api/invites/${encodeURIComponent(code)}`).then(
      (s) => (state = s),
      () => (state = { valid: false, site: '' }),
    );
  });
</script>

{#if state?.valid}
  <AuthShell title="Вас пригласили" subtitle={`Смотрите фильмы и сериалы «${state.site}» онлайн`}>
    <RegisterForm invite={code} askInvite={false} />
  </AuthShell>
{:else if state}
  <AuthShell title="Приглашение не действует" subtitle="Ссылка устарела или уже использована. Попросите новую у администрации Nova.">
    <form><a class="btn block" href="/login">Войти</a></form>
  </AuthShell>
{/if}
