<script lang="ts">
  import { api, del, errorText, patch } from '$lib/api';
  import Poster from '$lib/discover/Poster.svelte';
  import { MAX_FAVORITES as MAX } from '$lib/social/favorites';
  import PosterRibbon from '$lib/social/PosterRibbon.svelte';
  import { session } from '$lib/session.svelte';
  import type { MarkItem, ProfileDecor } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  // Оформление личной страницы: обложка, свой цвет, «о себе», до шести любимых фильмов
  // (выбираются из «Буду смотреть» и «Просмотрел» — с сервиса и из Обзора).
  let { profileId }: { profileId: number } = $props();

  const COLORS = ['#f5f5f7', '#ff453a', '#ff9f0a', '#ffd60a', '#30d158', '#64d2ff', '#0a84ff', '#5e5ce6', '#bf5af2', '#ff375f'];

  let decor = $state<ProfileDecor | null>(null);
  let pool = $state<MarkItem[]>([]);
  let bio = $state('');
  let suggest = $state<string[]>([]);
  let director = $state('');
  let busy = $state(false);

  const keyOf = (i: { card?: { id: number }; ext?: { id: string } }) => (i.card ? `t:${i.card.id}` : `x:${i.ext!.id}`);
  const favKeys = $derived(decor?.favorites.map((f) => f.key) ?? []);

  function load() {
    api<{ decor: ProfileDecor; stats: { directors: { name: string }[] } }>(`/api/members/${profileId}`).then((r) => {
      decor = r.decor;
      bio = r.decor.bio ?? '';
      suggest = r.stats.directors.map((d) => d.name);
    }, () => {});
    api<{ want: MarkItem[]; watched: MarkItem[] }>('/api/me/marks').then((m) => {
      const seen = new Set<string>();
      pool = [...m.watched, ...m.want].filter((i) => !seen.has(keyOf(i)) && seen.add(keyOf(i)));
    }, () => {});
  }
  $effect(load);

  async function save(body: Record<string, unknown>, note = '') {
    try {
      await patch('/api/me/profile', body);
      if (note) toast(note);
    } catch (x) {
      toast(errorText(x));
      load();
    }
  }

  function setAccent(c: string | null) {
    if (!decor) return;
    decor.accent = c;
    save({ accent: c ?? '' });
  }

  function toggleFav(i: MarkItem) {
    if (!decor) return;
    const k = keyOf(i);
    if (favKeys.includes(k)) decor.favorites = decor.favorites.filter((f) => f.key !== k);
    else if (decor.favorites.length >= MAX) return void toast(`Любимых — не больше ${MAX}`);
    else decor.favorites = [...decor.favorites, { key: k, card: i.card, ext: i.ext }];
    const keys = decor.favorites.map((f) => f.key);
    save({ favorites: keys });
    if (session.me) session.me.favorites = keys;
  }

  // Любимые режиссёры: из тех, чьи фильмы смотрел, или любое имя вручную.
  function setDirectors(list: string[]) {
    if (!decor) return;
    if (list.length > MAX) return void toast(`Любимых режиссёров — не больше ${MAX}`);
    decor.directors = list;
    save({ directors: list });
  }
  function addDirector(e?: SubmitEvent) {
    e?.preventDefault();
    const n = director.trim();
    if (!decor || !n) return;
    if (!decor.directors.some((d) => d.toLowerCase() === n.toLowerCase())) setDirectors([...decor.directors, n]);
    director = '';
  }

  function setKind(k: 'ribbon' | 'image') {
    if (!decor) return;
    decor.coverKind = k;
    save({ coverKind: k });
  }

  async function pickCover(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const f = input.files?.[0];
    input.value = '';
    if (!f || !decor) return;
    if (f.size > 15 * 1024 * 1024) return void toast('Картинка больше 15 МБ');
    busy = true;
    try {
      const r = await api<{ cover: string }>('/api/me/cover', { method: 'PUT', body: f });
      decor.cover = r.cover;
      decor.coverKind = 'image';
    } catch (x) {
      toast(errorText(x));
    } finally {
      busy = false;
    }
  }

  async function dropCover() {
    if (!decor) return;
    decor.cover = null;
    decor.coverKind = 'ribbon';
    await del('/api/me/cover').catch(load);
  }
</script>

<section class="card pane" aria-labelledby="decor">
  <h2 id="decor">Оформление страницы</h2>
  {#if decor}
    <div class="kinds" role="radiogroup" aria-label="Обложка">
      <button class="chip" role="radio" aria-checked={decor.coverKind === 'ribbon'} class:on={decor.coverKind === 'ribbon'} onclick={() => setKind('ribbon')}>Лента просмотренного</button>
      <button class="chip" role="radio" aria-checked={decor.coverKind === 'image'} class:on={decor.coverKind === 'image'} disabled={!decor.cover} onclick={() => setKind('image')}>Своя картинка</button>
    </div>
    <div class="cover" style:--acc={decor.accent ?? '#3a3a3c'}>
      {#if decor.coverKind === 'image' && decor.cover}<img src={decor.cover} alt="" decoding="async" />{:else}<PosterRibbon posters={decor.ribbon} accent={decor.accent} />{/if}
      <div class="cover-acts">
        <label class="btn sm" class:busy>
          <Icon name="image" size={14} />
          {busy ? 'Загружаем…' : decor.cover ? 'Другая обложка' : 'Обложка'}
          <input type="file" accept="image/jpeg,image/png,image/webp" onchange={pickCover} hidden />
        </label>
        {#if decor.cover}<button class="btn sm" onclick={dropCover}>Убрать</button>{/if}
      </div>
    </div>

    <div class="field">
      <span>Цвет</span>
      <div class="swatches" role="radiogroup" aria-label="Цвет профиля">
        <button role="radio" aria-checked={!decor.accent} aria-label="Без цвета" class="sw none" class:on={!decor.accent} onclick={() => setAccent(null)}><Icon name="close" size={14} /></button>
        {#each COLORS as c (c)}
          <button role="radio" aria-checked={decor.accent === c} aria-label="Цвет {c}" class="sw" class:on={decor.accent === c} style:background={c} onclick={() => setAccent(c)}></button>
        {/each}
      </div>
    </div>

    <form class="bio" onsubmit={(e) => (e.preventDefault(), save({ bio }, 'Сохранено'))}>
      <label class="field">
        <span>О себе <small class="faint">{bio.length}/300</small></span>
        <textarea class="input" bind:value={bio} maxlength="300" rows="3" placeholder="Что любите смотреть, любимые режиссёры…"></textarea>
      </label>
      <button class="btn">Сохранить</button>
    </form>

    <div class="field">
      <span>Любимые режиссёры <small class="faint">{decor.directors.length}/{MAX}</small></span>
      {#if decor.directors.length}
        <div class="tags">
          {#each decor.directors as d (d)}
            <span class="tag">{d}<button aria-label="Убрать {d}" onclick={() => setDirectors(decor!.directors.filter((x) => x !== d))}><Icon name="close" size={12} /></button></span>
          {/each}
        </div>
      {/if}
      <form class="line" onsubmit={addDirector}>
        <input class="input" bind:value={director} maxlength="80" placeholder="Имя режиссёра" aria-label="Режиссёр" />
        <button class="btn" disabled={!director.trim() || decor.directors.length >= MAX}>Добавить</button>
      </form>
      {#if suggest.filter((n) => !decor!.directors.includes(n)).length}
        <div class="tags">
          <small class="muted">Из того, что смотрели:</small>
          {#each suggest.filter((n) => !decor!.directors.includes(n)).slice(0, 8) as n (n)}
            <button class="chip" disabled={decor.directors.length >= MAX} onclick={() => setDirectors([...decor!.directors, n])}>+ {n}</button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="field">
      <span>Любимое <small class="faint">{decor.favorites.length}/{MAX}</small></span>
      {#if pool.length}
        <div class="pool">
          {#each pool as i (keyOf(i))}
            <div class="pick" class:on={favKeys.includes(keyOf(i))}>
              <Poster card={i.ext} local={i.card ?? null} />
              <button class="star" aria-pressed={favKeys.includes(keyOf(i))} aria-label="В любимое" title="В любимое" onclick={() => toggleFav(i)}>
                <Icon name="heart" size={15} />
              </button>
            </div>
          {/each}
        </div>
      {:else}
        <p class="muted small">Отметьте фильмы «Буду смотреть» или «Просмотрел» — их можно будет выбрать сюда.</p>
      {/if}
    </div>
  {:else}
    <div class="ph skeleton"></div>
  {/if}
</section>

<style>
  .pane {
    display: grid;
    gap: 18px;
    padding: 22px;
    margin-bottom: 18px;
  }
  h2 {
    font-size: 17px;
  }
  .cover {
    position: relative;
    height: 140px;
    border-radius: 14px;
    overflow: hidden;
    background:
      radial-gradient(120% 140% at 0% 0%, color-mix(in srgb, var(--acc) 80%, transparent), transparent 60%),
      linear-gradient(135deg, color-mix(in srgb, var(--acc) 45%, #111), #111);
  }
  .cover img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px 4px 12px;
    border-radius: 999px;
    background: var(--surface-2);
    font-size: 13px;
    font-weight: 600;
    animation: rise-in 0.3s ease-out both;
  }
  .tag button {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    color: var(--muted);
    cursor: pointer;
  }
  .line {
    display: flex;
    gap: 8px;
  }
  .line .input {
    flex: 1;
    min-width: 0;
  }
  .cover-acts {
    position: absolute;
    right: 10px;
    bottom: 10px;
    display: flex;
    gap: 6px;
  }
  .busy {
    opacity: 0.6;
    pointer-events: none;
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .sw {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 0;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.15);
    transition: transform 0.15s;
  }
  .sw:hover {
    transform: scale(1.1);
  }
  .sw.on {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 4px var(--text);
  }
  .sw.none {
    display: grid;
    place-items: center;
    background: var(--surface-2);
    color: var(--muted);
  }
  .bio {
    display: grid;
    gap: 10px;
  }
  .bio .btn {
    justify-self: start;
  }
  textarea {
    resize: vertical;
    min-height: 80px;
    font: inherit;
  }
  .pool {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 96px), 1fr));
    gap: 14px 10px;
  }
  .pick {
    position: relative;
    min-width: 0;
  }
  .pick :global(.sub) {
    display: none;
  }
  .star {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.7);
    color: #fff;
    cursor: pointer;
    z-index: 1;
  }
  .pick.on .star {
    background: #ff375f;
  }
  .small {
    font-size: 13px;
    margin: 0;
  }
  .ph {
    height: 200px;
    border-radius: 12px;
  }
</style>
