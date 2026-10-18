<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import { page } from '$app/state';
  import { api, errorText } from '$lib/api';
  import { ago } from '$lib/fmt';
  import Poster from '$lib/discover/Poster.svelte';
  import FriendButton from '$lib/social/FriendButton.svelte';
  import PosterWall from '$lib/social/PosterWall.svelte';
  import ProfileStats from '$lib/social/ProfileStats.svelte';
  import StarsShow from '$lib/social/StarsShow.svelte';
  import type { Card, MiniTitle, Person, ProfileDecor, ProfileStats as Stats, Relation } from '$lib/types';
  import { placeholder } from '$lib/ui/art';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';

  interface Profile {
    person: Person;
    me: boolean;
    relation: Relation | null;
    stats: Stats;
    decor: ProfileDecor;
    reviews: { id: number; stars: number | null; body: string; createdAt: number; title: MiniTitle }[];
    watched: Card[];
  }

  let p = $state<Profile | null>(null);
  let error = $state('');
  const id = $derived(Number(page.params.id));

  $effect(() => {
    const want = id;
    p = null;
    error = '';
    api<Profile>(`/api/members/${want}`).then((r) => want === id && (p = r), (e) => (error = errorText(e)));
  });
</script>

<div class="page narrow">
  {#if error}
    <div class="empty"><Icon name="info" size={36} /> {error}</div>
  {:else if p}
    {#if p.decor.coverKind === 'image' && p.decor.cover}
      <div class="cover" style:--acc={p.decor.accent ?? '#3a3a3c'}>
        <img use:fadeIn src={p.decor.cover} alt="" decoding="async" />
      </div>
    {:else}
      <!-- Фон всей страницы — постеры просмотренного плывут вверх; сверху — пустое место под имя. -->
      <PosterWall posters={p.decor.ribbon} accent={p.decor.accent} page />
      <div class="cover open"></div>
    {/if}
    <header class="head">
      <span class="ring" style:--acc={p.decor.accent ?? 'transparent'}><Avatar avatar={p.person.avatar} photo={p.person.photo} size={112} /></span>
      <div class="who">
        <h1>{p.person.name}</h1>
        {#if p.decor.bio}<p class="bio">{p.decor.bio}</p>{/if}
        {#if p.me}
          <a class="btn sm" href="/profile"><Icon name="edit" size={14} /> Оформить профиль</a>
        {:else if p.relation}
          <FriendButton id={p.person.id} bind:relation={p.relation} />
        {/if}
      </div>
    </header>

    {#if p.decor.favorites.length}
      <section aria-labelledby="f">
        <h2 id="f">Любимые фильмы</h2>
        <ol class="favs">
          {#each p.decor.favorites as f, i (f.key)}
            <li class="rise" style:--i={i}><span class="num">{i + 1}</span><Poster card={f.ext} local={f.card ?? null} /></li>
          {/each}
        </ol>
      </section>
    {/if}

    {#if p.decor.directors.length}
      <section aria-labelledby="d">
        <h2 id="d">Любимые режиссёры</h2>
        <ul class="dirs">
          {#each p.decor.directors as d, i (d)}
            <li class="rise" style:--i={i}><a href="/search?q={encodeURIComponent(d)}">{d}</a></li>
          {/each}
        </ul>
      </section>
    {/if}

    <section aria-labelledby="st">
      <h2 id="st">Статистика</h2>
      <ProfileStats stats={p.stats} accent={p.decor.accent} />
    </section>

    {#if p.watched.length}
      <section aria-labelledby="w">
        <h2 id="w">Просмотрено</h2>
        <ul class="posters">
          {#each p.watched as c (c.id)}
            <li>
              <a href="/title/{c.id}" title={c.title} style:background={c.poster ? undefined : placeholder(c.title)}>
                {#if c.poster}<img use:fadeIn src={c.poster} alt={c.title} loading="lazy" decoding="async" width="342" height="513" />{:else}<span>{c.title}</span>{/if}
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section aria-labelledby="r">
      <h2 id="r">Отзывы</h2>
      {#if p.reviews.length}
        <ul class="reviews">
          {#each p.reviews as r (r.id)}
            <li class="rv">
              <a class="mini" href="/title/{r.title.id}" style:background={r.title.poster ? undefined : placeholder(r.title.title)}>
                {#if r.title.poster}<img use:fadeIn src={r.title.poster} alt="" loading="lazy" decoding="async" width="56" height="84" />{/if}
              </a>
              <div class="body">
                <a href="/title/{r.title.id}"><b>{r.title.title}</b>{#if r.title.year} <span class="faint">{r.title.year}</span>{/if}</a>
                <div class="line">
                  {#if r.stars}<StarsShow value={r.stars} />{/if}
                  <small class="faint">{ago(r.createdAt)}</small>
                </div>
                {#if r.body}<p>{r.body}</p>{/if}
              </div>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted">Отзывов пока нет.</p>
      {/if}
    </section>
  {:else}
    <div class="sk skeleton"></div>
  {/if}
</div>

<style>
  /* Свой слой (z-index 0): фон-стена с z-index -1 лежит под содержимым, но над фоном body.
     Появление — только прозрачностью: transform сдвинул бы position: fixed у стены. */
  .narrow {
    position: relative;
    z-index: 0;
    max-width: 880px;
    margin: 0 auto;
    animation-name: fade-in;
  }
  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
  /* Обложка: своя картинка или мягкий градиент своего цвета; аватар заходит на её край. */
  .cover {
    position: relative;
    height: clamp(140px, 22vw, 220px);
    margin: -8px 0 0;
    border-radius: 20px;
    overflow: hidden;
    background:
      radial-gradient(120% 140% at 0% 0%, color-mix(in srgb, var(--acc) 80%, transparent), transparent 60%),
      linear-gradient(135deg, color-mix(in srgb, var(--acc) 45%, #111), #111);
  }
  .cover.open {
    background: none;
  }
  .cover img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .head {
    display: flex;
    align-items: flex-end;
    flex-wrap: wrap;
    gap: 16px 24px;
    margin: -56px 0 30px;
    padding: 0 20px;
    position: relative;
    animation: rise 0.45s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .ring {
    display: block;
    padding: 4px;
    border-radius: 50%;
    background: var(--bg);
    box-shadow: 0 0 0 3px var(--acc);
  }
  .bio {
    color: var(--muted);
    white-space: pre-line;
    overflow-wrap: anywhere;
    max-width: 60ch;
  }
  .favs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 14px;
  }
  .favs li {
    position: relative;
    min-width: 0;
  }
  /* Номер, как в «Топ-5» на IMDb: крупная цифра поверх угла постера. */
  .num {
    position: absolute;
    z-index: 1;
    left: 8px;
    top: 4px;
    font-size: 30px;
    font-weight: 800;
    line-height: 1;
    color: #fff;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.85);
    pointer-events: none;
  }
  .dirs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .dirs a {
    display: inline-block;
    padding: 8px 16px;
    border-radius: 999px;
    background: var(--surface);
    font-weight: 600;
    font-size: 14px;
    transition: background 0.2s;
  }
  .dirs a:hover {
    background: var(--surface-2);
  }
  @media (max-width: 640px) {
    .head {
      padding: 0 8px;
    }
    .favs {
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 10px;
    }
  }
  .who {
    display: grid;
    gap: 8px;
    justify-items: start;
    min-width: 0;
  }
  h1 {
    font-size: clamp(28px, 5vw, 42px);
    font-weight: 800;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }
  .who p {
    margin: 0;
  }
  section {
    margin-bottom: 34px;
  }
  h2 {
    font-size: 17px;
    font-weight: 700;
    margin-bottom: 14px;
  }
  .posters {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
    gap: 10px;
  }
  .posters a {
    display: grid;
    align-items: end;
    aspect-ratio: 2 / 3;
    border-radius: 10px;
    overflow: hidden;
    transition: transform 0.2s;
  }
  .posters a:hover {
    transform: translateY(-3px);
  }
  .posters img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .posters span {
    padding: 8px;
    font-size: 12px;
    font-weight: 700;
  }
  .reviews {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .rv {
    display: grid;
    grid-template-columns: 56px minmax(0, 1fr);
    gap: 14px;
    padding: 14px;
    border-radius: 14px;
    background: #1a1a1c;
  }
  .mini {
    display: block;
    width: 56px;
    aspect-ratio: 2 / 3;
    border-radius: 8px;
    overflow: hidden;
  }
  .mini img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .body {
    display: grid;
    gap: 4px;
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .body p {
    margin: 2px 0 0;
    font-size: 13.5px;
    color: #d4d4d8;
    white-space: pre-line;
    overflow-wrap: anywhere;
  }
  .sk {
    height: 300px;
    border-radius: 16px;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
  }
</style>
