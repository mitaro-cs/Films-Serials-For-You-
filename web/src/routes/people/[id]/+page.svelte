<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import { page } from '$app/state';
  import { api, errorText } from '$lib/api';
  import { ago, count, plural } from '$lib/fmt';
  import Poster from '$lib/discover/Poster.svelte';
  import PosterRow from '$lib/discover/PosterRow.svelte';
  import FriendButton from '$lib/social/FriendButton.svelte';
  import PosterWall from '$lib/social/PosterWall.svelte';
  import ProfileStats from '$lib/social/ProfileStats.svelte';
  import StarsShow from '$lib/social/StarsShow.svelte';
  import type { Card, MiniTitle, NowItem, Person, ProfileDecor, ProfileStats as Stats, Relation } from '$lib/types';
  import { placeholder } from '$lib/ui/art';
  import Avatar from '$lib/ui/Avatar.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import type { IconName } from '$lib/ui/icons';

  // Страница человека - как страница фильма: на фоне обложка (своя картинка или постеры
  // просмотренного плывут вверх), крупный портрет вместо постера, имя и «о себе», цифры и любимые
  // жанры; справа - панель с друзьями и кнопками. Ниже лист: сейчас смотрит, любимые, режиссёры,
  // статистика, просмотренное, отзывы.
  interface Profile {
    person: Person;
    me: boolean;
    relation: Relation | null;
    stats: Stats;
    decor: ProfileDecor;
    reviews: {
      id: number;
      stars: number | null;
      body: string;
      createdAt: number;
      title?: MiniTitle;
      ext?: { id: string; kind: string; title: string; year: number | null; poster: string | null };
    }[];
    watched: Card[];
    watching: NowItem[];
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

  const hours = $derived(p ? (p.stats.hours >= 10 ? Math.round(p.stats.hours) : p.stats.hours) : 0);
  // Цифры - капсулами с иконками; значки - за что-то (часы, сериалы, отзывы, друзья).
  const facts = $derived<{ icon: IconName; text: string }[]>(
    p
      ? [
          { icon: 'clock', text: `${hours} ${plural(Math.floor(p.stats.hours), 'час', 'часа', 'часов')}` },
          { icon: 'film', text: count(p.stats.movies, 'фильм', 'фильма', 'фильмов') },
          { icon: 'tv', text: count(p.stats.episodes, 'серия', 'серии', 'серий') },
        ]
      : [],
  );
  const badges = $derived.by(() => {
    if (!p) return [];
    const s = p.stats;
    const out: { icon: IconName; label: string; hint: string }[] = [];
    if (s.hours >= 50) out.push({ icon: 'sparkle', label: 'Киноман', hint: 'больше 50 часов просмотра' });
    if (s.series >= 3) out.push({ icon: 'tv', label: 'Марафонец', hint: 'смотрел 3 сериала и больше' });
    if (s.reviews >= 5) out.push({ icon: 'edit', label: 'Критик', hint: '5 отзывов и больше' });
    if (s.friends >= 5) out.push({ icon: 'users', label: 'Душа компании', hint: '5 друзей и больше' });
    if (s.outside >= 10) out.push({ icon: 'globe', label: 'Исследователь', hint: '10 отметок в Обзоре' });
    if (!out.length) out.push({ icon: 'play', label: 'Новичок', hint: 'всё только начинается' });
    return out;
  });
  const link = (r: Profile['reviews'][number]) => (r.ext ? `/ext/${r.ext.id}` : `/title/${r.title?.id}`);
  const what = (r: Profile['reviews'][number]) => r.ext ?? r.title!;
</script>

{#if error}
  <div class="page empty"><Icon name="info" size={36} /> {error}</div>
{:else if p}
  <section class="hero" style:--acc={p.decor.accent ?? '#3a3a3c'}>
    {#if p.decor.coverKind === 'image' && p.decor.cover}
      <div class="bg" aria-hidden="true"><img use:fadeIn src={p.decor.cover} alt="" decoding="async" /></div>
    {:else}
      <PosterWall posters={p.decor.ribbon} accent={p.decor.accent} />
    {/if}
    <div class="wrap">
      <div class="portrait">
        <span class="halo" aria-hidden="true"></span>
        <span class="face">
          {#if p.person.photo}
            <img use:fadeIn src={p.person.photo} alt={p.person.name} width="256" height="256" decoding="async" />
          {:else}
            <Avatar avatar={p.person.avatar} size={132} />
          {/if}
        </span>
        {#if p.watching.length}<span class="live" title="Сейчас смотрит"></span>{/if}
      </div>

      <div class="info">
        <h1>{p.person.name}</h1>
        <ul class="facts">{#each facts as f, i (f.text)}<li style:--i={i}><Icon name={f.icon} size={15} /> {f.text}</li>{/each}</ul>
        <ul class="badges" aria-label="Значки">
          {#each badges as b, i (b.label)}<li style:--i={i} title={b.hint}><Icon name={b.icon} size={14} /> {b.label}</li>{/each}
        </ul>
        {#if p.decor.bio}<p class="bio">{p.decor.bio}</p>{/if}
        {#if p.stats.genres.length}
          <div class="genres">
            {#each p.stats.genres.slice(0, 4) as g (g.genre)}<a class="pill" href="/search?q={encodeURIComponent(g.genre)}">{g.genre}</a>{/each}
          </div>
        {/if}
      </div>

      <aside class="panel" aria-label="Действия">
        <div class="box">
          <div class="nums">
            <div><b>{p.stats.watched ?? p.watched.length}</b><small>{plural(p.stats.watched ?? 0, 'просмотр', 'просмотра', 'просмотров')}</small></div>
            <div><b>{p.stats.reviews}</b><small>{plural(p.stats.reviews, 'отзыв', 'отзыва', 'отзывов')}</small></div>
            <div><b>{p.stats.friends}</b><small>{plural(p.stats.friends, 'друг', 'друга', 'друзей')}</small></div>
          </div>
        </div>
        {#if p.me}
          <a class="btn block" href="/profile"><Icon name="edit" size={14} /> Оформить профиль</a>
          <a class="btn block" href="/people"><Icon name="users" size={14} /> Найти друзей</a>
        {:else if p.relation}
          <FriendButton id={p.person.id} bind:relation={p.relation} />
          {#if p.relation === 'friend'}
            <a class="btn block" href="/chats/{p.person.id}"><Icon name="party" size={14} /> Позвать смотреть вместе</a>
          {/if}
        {/if}
      </aside>
    </div>
  </section>

  <main class="sheet">
    <section class="block" aria-labelledby="st">
      <h2 id="st">Статистика просмотра</h2>
      <ProfileStats stats={p.stats} accent={p.decor.accent} />
      <p class="seen-links">
        <a class="btn sm" href="/people/{p.person.id}/watched?kind=movie"><Icon name="film" size={14} /> Какие фильмы смотрел</a>
        <a class="btn sm" href="/people/{p.person.id}/watched?kind=series"><Icon name="tv" size={14} /> Какие сериалы смотрел</a>
      </p>
    </section>
    {#if p.watching.length}
      <div class="block now">
        <PosterRow title="Сейчас смотрит" items={p.watching.map((n) => ({ card: n.card, ext: n.ext, label: n.label }))} />
      </div>
    {/if}

    {#each [{ kind: 'movie', label: 'Любимые фильмы' }, { kind: 'series', label: 'Любимые сериалы' }] as g (g.kind)}
      {@const favs = p.decor.favorites.filter((f) => (f.card ?? f.ext)?.kind === g.kind)}
      {#if favs.length}
        <section class="block" aria-labelledby="f-{g.kind}">
          <h2 id="f-{g.kind}">{g.label}</h2>
          <ol class="favs">
            {#each favs as f, i (f.key)}
              <li class="rise" style:--i={i}><span class="num">{i + 1}</span><Poster card={f.ext} local={f.card ?? null} /></li>
            {/each}
          </ol>
        </section>
      {/if}
    {/each}

    {#if p.decor.directors.length}
      <section class="block" aria-labelledby="d">
        <h2 id="d">Любимый режиссёр</h2>
        <ul class="dirs">
          {#each p.decor.directors as d, i (d)}
            {@const ph = p.decor.directorsInfo?.find((x) => x.name === d)?.photo}
            <li class="rise" style:--i={i}>
              <a class="dir" href="/person?name={encodeURIComponent(d)}&role=director">
                <span class="dph" style:background={ph ? undefined : placeholder(d)}>
                  {#if ph}<img use:fadeIn src={ph} alt="" loading="lazy" decoding="async" width="96" height="96" />{:else}{d.split(/\s+/).slice(0, 2).map((w) => w[0]).join('')}{/if}
                </span>
                <span class="dn">{d}</span>
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/if}


    {#if p.decor.likedPeople?.length}
      <section class="block" aria-labelledby="lp">
        <h2 id="lp">Нравятся актёры и режиссёры</h2>
        <ul class="dirs">
          {#each p.decor.likedPeople as lp, i (lp.name + lp.role)}
            <li class="rise" style:--i={i}>
              <a class="dir" href="/person?name={encodeURIComponent(lp.name)}&role={lp.role}">
                <span class="dph" style:background={lp.photo ? undefined : placeholder(lp.name)}>
                  {#if lp.photo}<img use:fadeIn src={lp.photo} alt="" loading="lazy" decoding="async" width="96" height="96" />{:else}{lp.name.split(/\s+/).slice(0, 2).map((w) => w[0]).join('')}{/if}
                </span>
                <span class="dn">{lp.name}<small class="faint">{lp.role === 'actor' ? 'Актёр' : 'Режиссёр'}</small></span>
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if p.watched.length}
      <section class="block" aria-labelledby="w">
        <h2 id="w">Просмотрено</h2>
        <ul class="posters">
          {#each p.watched as c, i (c.id)}
            <li class="rise" style:--i={i}>
              <a href="/title/{c.id}" title={c.title} style:background={c.poster ? undefined : placeholder(c.title)}>
                {#if c.poster}<img use:fadeIn src={c.poster} alt={c.title} loading="lazy" decoding="async" width="342" height="513" />{:else}<span>{c.title}</span>{/if}
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section class="block" aria-labelledby="r">
      <h2 id="r">Отзывы</h2>
      {#if p.reviews.length}
        <ul class="reviews">
          {#each p.reviews as r (r.id)}
            {@const t = what(r)}
            <li class="rv">
              <a class="mini" href={link(r)} style:background={t.poster ? undefined : placeholder(t.title)}>
                {#if t.poster}<img use:fadeIn src={t.poster} alt="" loading="lazy" decoding="async" width="56" height="84" />{/if}
              </a>
              <div class="body">
                <a href={link(r)}><b>{t.title}</b>{#if t.year} <span class="faint">{t.year}</span>{/if}</a>
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
  </main>
{:else}
  <div class="sk-hero skeleton" aria-busy="true"></div>
{/if}

<style>
  /* Та же раскладка, что у страницы фильма: большой фон, «постер» - портрет, панель справа. */
  .hero {
    position: relative;
    padding: calc(var(--header) + clamp(110px, 18vw, 220px)) var(--gutter) 34px;
    overflow: hidden;
    isolation: isolate;
  }
  .hero > :global(.wall) {
    z-index: -1;
  }
  .bg {
    position: absolute;
    inset: 0;
    z-index: -1;
  }
  .bg img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: kenburns 1.4s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .bg::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, var(--bg) 0%, rgba(0, 0, 0, 0.55) 45%, rgba(0, 0, 0, 0.2) 100%);
  }
  @keyframes kenburns {
    from {
      opacity: 0;
      transform: scale(1.06);
    }
  }
  .wrap {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) minmax(0, 300px);
    gap: 28px 36px;
    align-items: end;
    max-width: 1440px;
    margin: 0 auto;
  }
  /* Портрет - круг в кольце цвета профиля; кольцо медленно вращается (только transform). */
  .portrait {
    position: relative;
    justify-self: center;
    width: clamp(120px, 12vw, 160px);
    aspect-ratio: 1;
    display: grid;
    place-items: center;
    animation: rise 0.6s 0.05s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .halo {
    position: absolute;
    inset: -6px;
    border-radius: 50%;
    background: conic-gradient(from 0deg, var(--acc), color-mix(in srgb, var(--acc) 20%, #fff), var(--acc), #111, var(--acc));
    animation: spin 12s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(1turn);
    }
  }
  .face {
    position: relative;
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    border-radius: 50%;
    overflow: hidden;
    background: #0c0c0e;
    box-shadow:
      0 0 0 4px #0c0c0e,
      0 24px 50px -16px rgba(0, 0, 0, 0.9);
  }
  .face img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .face :global(*) {
    max-width: 100%;
    max-height: 100%;
  }
  .live {
    position: absolute;
    right: 8%;
    bottom: 8%;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 4px #0c0c0e;
    animation: beat 1.8s ease-in-out infinite;
  }
  @keyframes beat {
    50% {
      transform: scale(0.82);
    }
  }
  .facts,
  .badges {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .facts li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.45);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
    font-size: 13px;
    font-weight: 600;
    animation: rise 0.5s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(var(--i) * 60ms + 200ms);
  }
  .badges li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 11px;
    border-radius: 8px;
    background: linear-gradient(135deg, color-mix(in srgb, var(--acc) 55%, #2a2a2e), #1c1c1f);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.02em;
    animation: badge 0.5s cubic-bezier(0.2, 0.9, 0.3, 1.5) both;
    animation-delay: calc(var(--i) * 80ms + 350ms);
  }
  @keyframes badge {
    from {
      opacity: 0;
      transform: scale(0.6) rotate(-6deg);
    }
  }
  .info {
    display: grid;
    gap: 12px;
    align-content: end;
    min-width: 0;
    animation: rise 0.6s 0.12s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  h1 {
    font-size: clamp(32px, 4.4vw, 58px);
    font-weight: 800;
    letter-spacing: -0.02em;
    line-height: 1.05;
    overflow-wrap: anywhere;
  }
  .bio {
    margin: 0;
    max-width: 62ch;
    color: #e4e4e7;
    font-size: 16px;
    white-space: pre-line;
    overflow-wrap: anywhere;
  }
  .genres {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .pill {
    padding: 6px 14px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
    font-size: 13px;
    font-weight: 600;
    transition: background 0.2s;
  }
  .pill:hover {
    background: rgba(255, 255, 255, 0.2);
  }
  .panel {
    display: grid;
    gap: 10px;
    animation: rise 0.6s 0.2s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  .box {
    padding: 16px;
    border-radius: 18px;
    background: rgba(28, 28, 30, 0.92);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }
  .nums {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    text-align: center;
  }
  .nums div {
    display: grid;
    gap: 2px;
  }
  .nums div + div {
    border-left: 1px solid var(--line);
  }
  .nums b {
    font-size: 24px;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
  }
  .nums small {
    font-size: 11.5px;
    color: var(--muted);
  }
  .panel .btn {
    height: 38px;
    border-radius: 999px;
    background: rgba(40, 40, 42, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.1);
    font-size: 13px;
  }
  .panel :global(.fb) {
    display: grid;
    grid-template-columns: 1fr;
  }
  .panel :global(.fb .btn) {
    width: 100%;
    border-radius: 999px;
  }

  /* ---------- лист с разделами ---------- */
  .sheet {
    width: min(1440px, 100% - 2 * var(--gutter));
    margin: 0 auto;
    padding: 8px clamp(18px, 4vw, 54px) 40px;
    background: #1a1a1c;
    border-radius: 22px;
  }
  .block {
    padding: 28px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  }
  .block:last-child {
    border-bottom: 0;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 17px;
    font-weight: 700;
    margin-bottom: 16px;
  }
  .now :global(.row) {
    margin-bottom: 0;
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
    z-index: 3;
    left: 8px;
    bottom: 58px;
    font-size: 34px;
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
    gap: 18px;
  }
  /* Режиссёр - круглое фото и имя под ним; нажать - его работы. */
  .dir {
    display: grid;
    justify-items: center;
    gap: 8px;
    width: 104px;
    text-align: center;
  }
  .dph {
    display: grid;
    place-items: center;
    width: 88px;
    height: 88px;
    border-radius: 50%;
    overflow: hidden;
    font-weight: 800;
    font-size: 22px;
    box-shadow:
      0 0 0 3px #1a1a1c,
      0 0 0 5px rgba(255, 255, 255, 0.14);
    transition: transform 0.3s cubic-bezier(0.2, 0.9, 0.3, 1.4);
  }
  .dph img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .dir:hover .dph {
    transform: translateY(-4px) scale(1.05);
  }
  .dn {
    font-size: 13px;
    font-weight: 600;
    line-height: 1.25;
    overflow-wrap: anywhere;
  }
  .posters {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    gap: 12px;
  }
  .posters a {
    display: grid;
    align-items: end;
    aspect-ratio: 2 / 3;
    border-radius: 10px;
    overflow: hidden;
    transition: transform 0.25s cubic-bezier(0.2, 0.9, 0.3, 1.3);
  }
  .posters a:hover {
    transform: translateY(-4px) scale(1.02);
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
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 380px), 1fr));
    gap: 12px;
  }
  .rv {
    display: grid;
    grid-template-columns: 56px minmax(0, 1fr);
    gap: 14px;
    padding: 14px;
    border-radius: 14px;
    background: #232325;
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
  .sk-hero {
    height: 70vh;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(14px);
    }
  }

  @media (max-width: 980px) {
    .wrap {
      grid-template-columns: auto minmax(0, 1fr);
    }
    .panel {
      grid-column: 1 / -1;
    }
  }
  /* Телефон: колонкой по центру - круг сверху, под ним имя, капсулы и значки. */
  @media (max-width: 640px) {
    .hero {
      padding-top: calc(var(--header) + 40px);
    }
    .wrap {
      grid-template-columns: minmax(0, 1fr);
      justify-items: center;
      gap: 18px;
      text-align: center;
    }
    .portrait {
      width: 112px;
    }
    .info {
      justify-items: center;
    }
    .facts,
    .badges,
    .genres {
      justify-content: center;
    }
    h1 {
      font-size: 30px;
    }
    .panel {
      width: 100%;
    }
    .favs {
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 10px;
    }
    .sheet {
      width: 100%;
      border-radius: 18px 18px 0 0;
    }
  }

  .sheet { max-width: 1800px; width: calc(100% - var(--gutter) * 2); background: var(--panel); border: 1px solid var(--line); border-radius: 26px; }
  @media(max-width: 640px) { .sheet { width: 100%; border-radius: 20px 20px 0 0; } }

  .seen-links {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin: 18px 0 0;
  }
</style>
