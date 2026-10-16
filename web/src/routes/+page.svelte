<script lang="ts">
  import { api, del } from '$lib/api';
  import type { Home } from '$lib/types';
  import Hero from '$lib/ui/Hero.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Poster from '$lib/ui/Poster.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Wide from '$lib/ui/Wide.svelte';
  import { session } from '$lib/session.svelte';

  let home = $state<Home | null>(null);
  let failed = $state(false);

  $effect(() => {
    api<Home>('/api/home').then((h) => (home = h), () => (failed = true));
  });

  async function forget(fileId: number) {
    if (!home) return;
    home.continue = home.continue.filter((c) => c.next.fileId !== fileId);
    await del(`/api/progress/${fileId}`).catch(() => {});
  }

  const empty = $derived(home && !home.hero.length && !home.rows.length);
  const HREF: Record<string, string> = { movies: '/movies', series: '/series', list: '/list', new: '/movies?sort=added' };
</script>

{#if home}
  {#if empty}
    <div class="page">
      <div class="empty first">
        <Icon name="film" size={48} />
        <h1>Здесь пока пусто</h1>
        {#if session.me?.manage}
          <p>Добавьте папку с фильмами и сериалами — постеры, описания и актёры подтянутся сами.</p>
          <a class="btn primary" href="/admin?tab=folders"><Icon name="folder" size={18} /> Добавить папку</a>
        {:else}
          <p>Хост ещё не добавил фильмы. Загляните позже.</p>
        {/if}
      </div>
    </div>
  {:else}
    {#if home.hero.length}
      <Hero items={home.hero} />
    {:else}
      <div style:height="calc(var(--header) + 12px)"></div>
    {/if}
    <div class="rows" class:lift={home.hero.length > 0}>
      {#if home.continue.length}
        <div style="--col: 300px; --col-sm: 240px">
          <Row title="Продолжить просмотр">
            {#each home.continue as item (item.id)}
              <Wide {item} onremove={() => forget(item.next.fileId)} />
            {/each}
          </Row>
        </div>
      {/if}
      {#each home.rows as row (row.key)}
        <Row
          title={row.title}
          href={HREF[row.key] ?? (row.key.startsWith('genre:') ? `/movies?genre=${encodeURIComponent(row.title)}&all=1` : undefined)}
        >
          {#each row.items as card (card.id)}
            <Poster {card} />
          {/each}
        </Row>
      {/each}
    </div>
  {/if}
{:else if failed}
  <div class="page empty"><Icon name="offline" size={40} /> Не удалось загрузить. Обновите страницу.</div>
{:else}
  <div class="sk-hero skeleton"></div>
{/if}

<style>
  .rows.lift {
    position: relative;
    margin-top: -56px;
  }
  .first h1 {
    font-size: 28px;
    color: var(--text);
  }
  .first p {
    max-width: 420px;
    margin: 0 0 8px;
  }
  .sk-hero {
    height: min(86vh, 820px);
  }
</style>
