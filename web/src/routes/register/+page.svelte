<script lang="ts">
  import { api } from '$lib/api';
  import type { Site } from '$lib/types';
  import AuthShell from '$lib/ui/AuthShell.svelte';
  import RegisterForm from '$lib/ui/RegisterForm.svelte';

  let site = $state<Site | null>(null);
  $effect(() => {
    api<Site>('/api/site').then((s) => (site = s), () => {});
  });
</script>

{#if site}
  <AuthShell
    title="Регистрация"
    subtitle={site.registration === 'open' ? `Добро пожаловать в «${site.name}»` : 'Введите код из приглашения хоста'}
  >
    <RegisterForm askInvite={site.registration !== 'open'} />
  </AuthShell>
{/if}
