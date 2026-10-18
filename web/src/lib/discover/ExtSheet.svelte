<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import { goto } from '$app/navigation';
  import { api, del, errorText, post, put } from '$lib/api';
  import { clock } from '$lib/fmt';
  import LinkCard from '$lib/links/LinkCard.svelte';
  import LinkFinder from '$lib/links/LinkFinder.svelte';
  import NewList from '$lib/social/NewList.svelte';
  import type { ExtDetail, FoundVideo } from '$lib/types';
  import { placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import Modal from '$lib/ui/Modal.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import { extChanged, extSheet } from './sheet.svelte';

  // Фильм из ленты IMDb: описание, «Буду смотреть»/«Просмотрел», в свой список, попросить
  // хоста добавить, посмотреть по ссылке (Rutube, VK) прямо в плеере. Есть у нас — «Смотреть».
  let open = $state(false);
  let d = $state<ExtDetail | null>(null);
  let error = $state('');
  let finding = $state(false);
  let creating = $state(false);
  let lists = $state(false);
  // Сам фильм на Rutube/VK Видео: нашёлся — смотрим там, просить хоста незачем.
  let online = $state<FoundVideo[] | null>(null);

  $effect(() => {
    const id = extSheet.id;
    if (!id) return;
    open = true;
    d = null;
    error = '';
    lists = false;
    online = null;
    load(id);
  });
  $effect(() => {
    if (!open) extSheet.id = null;
  });

  function load(id: string) {
    api<ExtDetail>(`/api/discover/title/${id}`).then(
      (r) => {
        if (extSheet.id !== id) return;
        d = r;
        if (!r.card.local && !r.links.length && online === null) lookOnline(r);
      },
      (x) => (error = errorText(x)),
    );
  }

  const norm = (s: string) =>
    s
      .toLowerCase()
      .replace(/ё/g, 'е')
      .replace(/[^\p{L}\p{N}]+/gu, ' ')
      .trim();

  /** Полный фильм (от 40 мин) или серия (от 15 мин), в названии которых есть название фильма. */
  async function lookOnline(r: ExtDetail) {
    const q = `${r.card.title}${r.card.year ? ` ${r.card.year}` : ''}`;
    const min = r.card.kind === 'series' ? 15 * 60 : 40 * 60;
    const found = await api<FoundVideo[]>(`/api/links/search?min=${min}&q=${encodeURIComponent(q)}`).catch(() => [] as FoundVideo[]);
    if (extSheet.id !== r.card.id) return;
    const names = [r.card.title, r.card.original].filter((n): n is string => !!n).map(norm);
    online = found.filter((f) => names.some((n) => norm(f.label).includes(n))).slice(0, 6);
  }

  async function playFound(f: FoundVideo) {
    if (!d) return;
    try {
      const r = await post<{ id: number }>('/api/links', {
        url: f.url,
        extId: d.card.id,
        label: f.label,
        thumb: f.thumbUrl,
        duration: f.duration,
      });
      open = false;
      goto(`/watch/link/${r.id}`);
    } catch (x) {
      toast(errorText(x));
    }
  }

  // «Хочу на Nova» — только когда фильма нет ни у нас, ни по ссылкам, ни на Rutube/VK.
  const canWish = $derived(!!d && !d.card.local && !d.links.length && online !== null && !online.length);

  const c = $derived(d?.card ?? extSheet.seed);

  async function mark(status: 'want' | 'watched') {
    if (!d) return;
    const before = d.card.mark;
    const next = before === status ? null : status;
    d.card.mark = next;
    try {
      await put(`/api/discover/title/${d.card.id}/mark`, { status: next });
      extChanged();
    } catch {
      d.card.mark = before;
    }
  }

  async function wish() {
    if (!d) return;
    const next = !d.card.wished;
    d.card.wished = next;
    try {
      await put(`/api/discover/title/${d.card.id}/wish`, { value: next });
      toast(next ? 'Хост увидит просьбу добавить этот фильм' : 'Просьба отменена');
    } catch {
      d.card.wished = !next;
    }
  }

  async function toggleList(l: { id: number; name: string; has: boolean }) {
    if (!d) return;
    l.has = !l.has;
    try {
      await put(`/api/lists/${l.id}/ext/${d.card.id}`, { value: l.has });
      toast(l.has ? `Добавили в «${l.name}»` : `Убрали из «${l.name}»`);
    } catch (x) {
      l.has = !l.has;
      toast(errorText(x));
    }
  }

  async function created(id: number) {
    if (!d) return;
    await put(`/api/lists/${id}/ext/${d.card.id}`, { value: true }).catch(() => {});
    open = true;
    load(d.card.id);
  }

  async function removeLink(id: number) {
    if (!d) return;
    await del(`/api/links/${id}`).catch((x) => toast(errorText(x)));
    load(d.card.id);
  }

  const query = $derived(c ? `${c.title}${c.year ? ` ${c.year}` : ''}` : '');
</script>

<Modal bind:open title={c?.title ?? 'Фильм'} wide>
  {#if error}
    <p class="error">{error}</p>
  {:else if c}
    <div class="top">
      <div class="poster" style:background={c.poster ? undefined : placeholder(c.title)}>
        {#if c.poster}<img use:fadeIn src={c.poster} alt="" width="342" height="513" />{/if}
      </div>
      <div class="info">
        <h3>{c.title}</h3>
        {#if c.original}<p class="orig">{c.original}</p>{/if}
        <p class="meta">
          {#if c.year}<span>{c.year}</span>{/if}
          <span>{c.kind === 'series' ? 'Сериал' : 'Фильм'}</span>
          {#if d?.runtime}<span>{Math.floor(d.runtime / 60) ? `${Math.floor(d.runtime / 60)} ч ` : ''}{d.runtime % 60} мин</span>{/if}
          {#each c.genres.slice(0, 3) as g (g)}<span>{g}</span>{/each}
        </p>
        {#if c.rating}<p class="rate"><b class="imdb">IMDb</b> {c.rating.toFixed(1)}{#if d?.votes}<span class="muted"> · {d.votes.toLocaleString('ru-RU')} оценок</span>{/if}</p>{/if}
        {#if d?.overview}<p class="text">{d.overview}</p>{:else if !d}<p class="text skeleton ph-text"></p>{/if}

        <div class="acts">
          {#if c.local}
            <a class="btn white" href="/title/{c.local}" onclick={() => (open = false)}><Icon name="play" size={16} /> Смотреть на Nova</a>
          {/if}
          <button class="btn" class:on={c.mark === 'want'} aria-pressed={c.mark === 'want'} disabled={!d} onclick={() => mark('want')}>
            <Icon name="bookmark" size={16} /> Буду смотреть
          </button>
          <button class="btn" class:on={c.mark === 'watched'} aria-pressed={c.mark === 'watched'} disabled={!d} onclick={() => mark('watched')}>
            <Icon name="check" size={16} /> Просмотрел
          </button>
          <button class="btn" aria-expanded={lists} disabled={!d} onclick={() => (lists = !lists)}><Icon name="plus" size={16} /> В список</button>
          {#if canWish || c.wished}
            <button class="btn" class:on={c.wished} aria-pressed={c.wished} disabled={!d} onclick={wish}>
              <Icon name="mail" size={16} /> {c.wished ? 'Попросили хоста' : 'Хочу на Nova'}
            </button>
          {/if}
        </div>
        {#if lists && d}
          <div class="lists">
            {#each d.lists as l (l.id)}
              <button class="li" class:on={l.has} role="menuitemcheckbox" aria-checked={l.has} onclick={() => toggleList(l)}>
                <span class="tick"><Icon name="check" size={13} stroke={3} /></span>{l.name}
              </button>
            {/each}
            <button class="li new" onclick={() => ((open = false), (creating = true))}><Icon name="plus" size={14} /> Новый список</button>
          </div>
        {/if}
      </div>
    </div>

    <section class="online" aria-label="Смотреть онлайн">
      <div class="head">
        <h4>Смотреть онлайн</h4>
        <button class="btn sm" disabled={!d} onclick={() => (finding = true)}><Icon name="search" size={14} /> Найти видео</button>
      </div>
      {#if d?.links.length}
        <div class="links">
          {#each d.links as l (l.id)}<LinkCard link={l} onremove={() => removeLink(l.id)} />{/each}
        </div>
      {:else if online?.length}
        <ul class="found">
          {#each online as f (f.url)}
            <li>
              <button onclick={() => playFound(f)} aria-label="Смотреть: {f.label}">
                <span class="art">
                  {#if f.thumb}<img use:fadeIn src={f.thumb} alt="" loading="lazy" decoding="async" />{/if}
                  <span class="site">{f.site === 'vk' ? 'VK Видео' : 'Rutube'}</span>
                  {#if f.duration}<span class="dur">{clock(f.duration)}</span>{/if}
                </span>
                <span class="name">{f.label}</span>
              </button>
            </li>
          {/each}
        </ul>
      {:else if d && !c.local && online === null}
        <p class="muted small">Ищем на Rutube и VK Видео…</p>
      {:else if d}
        <p class="muted small">Найдите фильм на Rutube или вставьте ссылку — он откроется прямо в плеере Nova.</p>
      {/if}
    </section>
  {/if}
</Modal>
<NewList bind:open={creating} oncreated={created} />
{#if d}
  <LinkFinder bind:open={finding} {query} extId={d.card.id} onadded={() => d && load(d.card.id)} />
{/if}

<style>
  .found {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 200px), 1fr));
    gap: 14px;
  }
  .found button {
    display: grid;
    gap: 6px;
    width: 100%;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .found .art {
    position: relative;
    display: block;
    aspect-ratio: 16 / 9;
    border-radius: 10px;
    overflow: hidden;
    background: var(--surface);
  }
  .found img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .found .site,
  .found .dur {
    position: absolute;
    bottom: 6px;
    padding: 1px 6px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.72);
    font-size: 11px;
    font-weight: 700;
  }
  .found .site {
    left: 6px;
  }
  .found .dur {
    right: 6px;
  }
  .found .name {
    font-size: 13.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .top {
    display: grid;
    grid-template-columns: 170px minmax(0, 1fr);
    gap: 22px;
  }
  .poster {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: 12px;
    overflow: hidden;
    background: var(--surface);
    align-self: start;
  }
  .poster img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .info {
    display: grid;
    gap: 8px;
    align-content: start;
    min-width: 0;
  }
  h3 {
    font-size: 26px;
    font-weight: 800;
    overflow-wrap: anywhere;
  }
  .orig {
    margin: 0;
    color: var(--muted);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 12px;
    margin: 0;
    font-size: 14px;
    color: var(--muted);
  }
  .rate {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-weight: 700;
  }
  .text {
    margin: 4px 0 0;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.88);
  }
  .ph-text {
    height: 66px;
    border-radius: 8px;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
  }
  .acts .btn.on {
    background: rgba(235, 235, 240, 0.95);
    color: #111;
  }
  .lists {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .li {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 12px;
    border: 0;
    border-radius: 999px;
    background: var(--surface-2);
    cursor: pointer;
    font-size: 13px;
  }
  .tick {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 5px;
    border: 1.5px solid var(--line-strong);
    color: transparent;
  }
  .li.on .tick {
    background: #fff;
    border-color: #fff;
    color: #000;
  }
  .online {
    margin-top: 26px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 12px;
  }
  h4 {
    margin: 0;
    font-size: 17px;
  }
  .links {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 200px), 1fr));
    gap: 16px 14px;
  }
  .small {
    font-size: 13px;
  }
  @media (max-width: 560px) {
    .top {
      grid-template-columns: 110px minmax(0, 1fr);
      gap: 14px;
    }
    h3 {
      font-size: 20px;
    }
  }
</style>
