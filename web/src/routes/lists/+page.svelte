<script lang="ts">
  import { goto } from '$app/navigation';
  import { api, errorText } from '$lib/api';
  import NewList from '$lib/social/NewList.svelte';
  import type { SharedList } from '$lib/types';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';

  // Совместные списки: с кем и что смотрим вместе.
  let lists = $state<SharedList[] | null>(null);
  let error = $state('');
  let creating = $state(false);

  $effect(() => {
    api<SharedList[]>('/api/lists').then((r) => (lists = r), (e) => (error = errorText(e)));
  });
</script>

<div class="page">
  <div class="head">
    <h1>Смотрим вместе</h1>
    <button class="btn primary" onclick={() => (creating = true)}><Icon name="plus" size={16} /> Новый список</button>
  </div>
  <p class="muted lead">Общие списки с друзьями: добавляйте фильмы и отмечайте, что уже посмотрели вместе.</p>

  {#if error}
    <div class="empty"><Icon name="info" size={36} /> {error}</div>
  {:else if lists === null}
    <div class="grid">{#each Array(3) as _, i (i)}<div class="ph skeleton"></div>{/each}</div>
  {:else if lists.length === 0}
    <div class="empty"><Icon name="users" size={40} /> Пока нет ни одного списка — создайте первый и позовите друга.</div>
  {:else}
    <div class="grid">
      {#each lists as l (l.id)}
        <a class="list" href="/lists/{l.id}">
          <span class="art" class:many={l.art.length > 1}>
            {#each l.art.slice(0, 4) as a, i (i)}<img src={a} alt="" loading="lazy" decoding="async" />{/each}
            {#if !l.art.length}<span class="none"><Icon name="film" size={30} /></span>{/if}
          </span>
          <span class="name">{l.name}</span>
          <span class="sub">
            <span class="faces">
              {#each l.members.slice(0, 4) as m (m.id)}<Avatar avatar={m.avatar} photo={m.photo} size={22} />{/each}
            </span>
            {l.count ? `${l.watched} из ${l.count} посмотрели` : 'пока пусто'}
          </span>
        </a>
      {/each}
    </div>
  {/if}
</div>
<NewList bind:open={creating} oncreated={(id) => goto(`/lists/${id}`)} />

<style>
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  h1 {
    font-size: clamp(28px, 4vw, 40px);
    font-weight: 800;
  }
  .lead {
    margin: 6px 0 24px;
  }
  .ph {
    aspect-ratio: 16 / 9;
    border-radius: 12px;
  }
  .list {
    display: grid;
    gap: 4px;
    min-width: 0;
  }
  .art {
    position: relative;
    display: grid;
    aspect-ratio: 16 / 9;
    margin-bottom: 8px;
    border-radius: 12px;
    overflow: hidden;
    background: var(--bg-3);
    transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .art.many {
    grid-template-columns: 1fr 1fr;
    grid-template-rows: 1fr 1fr;
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    min-height: 0;
  }
  .art.many img:first-child:nth-last-child(2),
  .art.many img:first-child:nth-last-child(2) ~ img {
    grid-row: span 2;
  }
  .art.many img:first-child:nth-last-child(3) {
    grid-row: span 2;
  }
  .none {
    display: grid;
    place-items: center;
    color: var(--faint);
  }
  .list:hover .art {
    transform: scale(1.03);
  }
  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-size: 13px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
  }
  .faces {
    display: flex;
    flex: none;
  }
  .faces :global(*:not(:first-child)) {
    margin-left: -6px;
  }
</style>
