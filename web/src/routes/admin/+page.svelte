<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api } from '$lib/api';
  import Access from '$lib/admin/Access.svelte';
  import Catalog from '$lib/admin/Catalog.svelte';
  import Folders from '$lib/admin/Folders.svelte';
  import Overview from '$lib/admin/Overview.svelte';
  import People from '$lib/admin/People.svelte';
  import Settings from '$lib/admin/Settings.svelte';
  import Version from '$lib/admin/Version.svelte';
  import Wishes from '$lib/admin/Wishes.svelte';
  import { badges } from '$lib/social/badges.svelte';
  import type { Status } from '$lib/admin/types';
  import { session } from '$lib/session.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import type { IconName } from '$lib/ui/icons';

  const TABS: [string, string, IconName][] = [
    ['overview', 'Обзор', 'pulse'],
    ['folders', 'Папки', 'folder'],
    ['catalog', 'Каталог', 'film'],
    ['people', 'Люди', 'users'],
    ['wishes', 'Просьбы', 'flag'],
    ['access', 'Доступ', 'globe'],
    ['settings', 'Настройки', 'sliders'],
    ['version', 'Версия', 'download'],
  ];

  const tab = $derived(page.url.searchParams.get('tab') ?? 'overview');
  let status = $state<Status | null>(null);

  function reload() {
    api<Status>('/api/admin/status').then((s) => (status = s), () => {});
  }
  $effect(reload);

  $effect(() => {
    if (session.me && !session.me.manage) goto('/', { replace: true });
  });
</script>

<div class="page admin">
  <aside>
    <h1>Управление</h1>
    <nav>
      {#each TABS as [id, label, icon] (id)}
        <a href="?tab={id}" class:on={tab === id} data-sveltekit-replacestate data-sveltekit-noscroll>
          <span class="ic"><Icon name={icon} size={18} /></span>{label}
          {#if id === 'version' && status?.update?.state === 'available'}<i class="new" aria-label="есть обновление"></i>{/if}
          {#if id === 'wishes' && badges.wishes}<i class="new" aria-label="есть просьбы"></i>{/if}
        </a>
      {/each}
    </nav>
    {#if status}<a class="faint ver" href="?tab=version">Версия {status.version}</a>{/if}
  </aside>
  <section>
    {#if status}
      {#if tab === 'overview'}<Overview {status} {reload} />
      {:else if tab === 'folders'}<Folders {status} window={!!session.me?.window} {reload} />
      {:else if tab === 'catalog'}<Catalog />
      {:else if tab === 'people'}<People />
      {:else if tab === 'wishes'}<Wishes />
      {:else if tab === 'access'}<Access />
      {:else if tab === 'settings'}<Settings />
      {:else if tab === 'version'}<Version {status} {reload} />
      {/if}
    {/if}
  </section>
</div>

<style>
  .admin {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 32px;
    max-width: 1200px;
    margin: 0 auto;
  }
  aside {
    position: sticky;
    top: calc(var(--header) + 24px);
    align-self: start;
  }
  h1 {
    font-size: 26px;
    margin-bottom: 18px;
  }
  nav {
    display: grid;
    gap: 4px;
  }
  nav a {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 12px;
    color: var(--muted);
    font-weight: 600;
    font-size: 14px;
  }
  nav a:hover {
    color: var(--text);
    background: var(--surface);
  }
  .ic {
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    border-radius: 10px;
    background: var(--surface);
  }
  nav a.on {
    color: var(--text);
  }
  nav a.on .ic {
    background: var(--accent-grad);
    color: var(--on-accent);
  }
  .ver {
    display: block;
    font-size: 12px;
    margin-top: 18px;
  }
  .new {
    width: 8px;
    height: 8px;
    margin-left: auto;
    border-radius: 50%;
    background: #ff5c7a;
  }
  @media (max-width: 860px) {
    .admin {
      /* minmax(0, …): лента разделов прокручивается, а не распирает страницу. */
      grid-template-columns: minmax(0, 1fr);
      gap: 18px;
    }
    aside {
      position: static;
    }
    nav {
      display: flex;
      overflow-x: auto;
      margin: 0 calc(var(--gutter) * -1);
      padding: 0 var(--gutter);
    }
    nav a {
      white-space: nowrap;
    }
    .ver {
      display: none;
    }
  }
</style>
