<script lang="ts">
  import { api, del, errorText, patch } from '$lib/api';
  import Poster from '$lib/discover/Poster.svelte';
  import type { MarkItem, ProfileDecor } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  // Оформление личной страницы: обложка, свой цвет, «о себе», до шести любимых фильмов
  // (выбираются из «Буду смотреть» и «Просмотрел» — с сервиса и из Обзора).
  let { profileId }: { profileId: number } = $props();

  const COLORS = ['#f5f5f7', '#ff453a', '#ff9f0a', '#ffd60a', '#30d158', '#64d2ff', '#0a84ff', '#5e5ce6', '#bf5af2', '#ff375f'];
  const MAX = 6;

  let decor = $state<ProfileDecor | null>(null);
  let pool = $state<MarkItem[]>([]);
  let bio = $state('');
  let busy = $state(false);

  const keyOf = (i: { card?: { id: number }; ext?: { id: string } }) => (i.card ? `t:${i.card.id}` : `x:${i.ext!.id}`);
  const favKeys = $derived(decor?.favorites.map((f) => f.key) ?? []);

  function load() {
    api<{ decor: ProfileDecor }>(`/api/members/${profileId}`).then((r) => {
      decor = r.decor;
      bio = r.decor.bio ?? '';
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
    save({ favorites: decor.favorites.map((f) => f.key) });
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
    } catch (x) {
      toast(errorText(x));
    } finally {
      busy = false;
    }
  }

  async function dropCover() {
    if (!decor) return;
    decor.cover = null;
    await del('/api/me/cover').catch(load);
  }
</script>

<section class="card pane" aria-labelledby="decor">
  <h2 id="decor">Оформление страницы</h2>
  {#if decor}
    <div class="cover" style:--acc={decor.accent ?? '#3a3a3c'}>
      {#if decor.cover}<img src={decor.cover} alt="" decoding="async" />{/if}
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
