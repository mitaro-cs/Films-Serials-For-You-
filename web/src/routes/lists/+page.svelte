<script lang="ts">
  import { goto } from '$app/navigation';
  import { api, errorText } from '$lib/api';
  import PosterRow from '$lib/discover/PosterRow.svelte';
  import { extSheet } from '$lib/discover/sheet.svelte';
  import NewList from '$lib/social/NewList.svelte';
  import type { MarkItem, SharedList } from '$lib/types';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';

  // Мои списки: «Буду смотреть» и «Просмотрел» (с сервиса и из ленты IMDb), ниже — общие
  // списки с друзьями.
  let lists = $state<SharedList[] | null>(null);
  let marks = $state<{ want: MarkItem[]; watched: MarkItem[] } | null>(null);
  let error = $state('');
  let creating = $state(false);

  $effect(() => {
    void extSheet.version;
    // Вместе: иначе общие списки сдвигались бы, когда приходят «Буду смотреть» (CLS).
    Promise.all([
      api<SharedList[]>('/api/lists'),
      api<{ want: MarkItem[]; watched: MarkItem[] }>('/api/me/marks').catch(() => ({ want: [], watched: [] })),
    ]).then(
      ([l, m]) => {
        marks = m;
        lists = l;
      },
      (e) => (error = errorText(e)),
    );
  });
</script>

<div class="page">
  <h1 class="top">Списки</h1>
  {#if error}
    <div class="empty"><Icon name="info" size={36} /> {error}</div>
  {:else if marks === null || lists === null}
    <div class="ph-row skeleton"></div>
  {:else}
    {#if marks.want.length}<PosterRow title="Буду смотреть" items={marks.want} />{/if}
    {#if marks.watched.length}<PosterRow title="Просмотрел" items={marks.watched} />{/if}
    {#if !marks.want.length && !marks.watched.length}
      <p class="muted hint">
        Отмечайте фильмы «Буду смотреть» и «Просмотрел» — на странице фильма или в <a href="/discover">Обзоре</a>, они соберутся здесь.
      </p>
    {/if}

  <div class="head">
    <h2>Смотрим вместе</h2>
    <button class="btn primary" onclick={() => (creating = true)}><Icon name="plus" size={16} /> Новый список</button>
  </div>
  <p class="muted lead">Общие списки с друзьями: добавляйте фильмы и отмечайте, что уже посмотрели вместе.</p>

  {#if lists.length === 0}
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
  .top {
    margin-bottom: 20px;
  }
  h2 {
    font-size: clamp(19px, 1.6vw, 24px);
    font-weight: 700;
  }
  .hint {
    margin: 0 0 32px;
  }
  .hint a {
    text-decoration: underline;
  }
  .ph-row {
    height: 280px;
    border-radius: 12px;
    margin-bottom: 30px;
  }
  .lead {
    margin: 6px 0 24px;
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
