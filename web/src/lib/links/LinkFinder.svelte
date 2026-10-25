<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import { goto } from '$app/navigation';
  import { untrack } from 'svelte';
  import { errorText, post } from '$lib/api';
  import { searchVideos, sourceName } from './search';
  import type { VideoLink } from '$lib/types';
  import { clock } from '$lib/fmt';
  import type { FoundVideo } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import Modal from '$lib/ui/Modal.svelte';

  // Видео с других сайтов: вставить ссылку или найти по названию на Rutube (и VK Видео, если
  // хост дал ключ). Сохранённое смотрится внутри нашего плеера.
  let {
    open = $bindable(false),
    query = '',
    titleId = null,
    extId = null,
    season = null,
    episode = null,
    saved = [],
    onadded,
  }: { open: boolean; query?: string; titleId?: number | null; extId?: string | null; season?: number | null; episode?: number | null; saved?: VideoLink[]; onadded?: () => void } = $props();

  let text = $state('');
  let found = $state<FoundVideo[] | null>(null);
  let busy = $state(false);
  let searching = $state(false);
  let error = $state('');
  let request = 0;
  let controller: AbortController | null = null;

  const isUrl = $derived(/^https?:\/\//i.test(text.trim()));

  // Только при открытии: поиск читает `text`, и без untrack каждая буква сбрасывала поле.
  $effect(() => {
    if (!open) {controller?.abort();return;}
    const q = query;
    untrack(() => {
      text = q;
      found = null;
      error = '';
      if (q.trim().length >= 2) search();
    });
  });

  // Поиск идёт до 8 с - вставленная за это время ссылка открывается, не дожидаясь его.
  async function search() {
    controller?.abort();controller=new AbortController();
    const ticket = ++request;
    searching = true;
    error = '';
    try {
      const params=new URLSearchParams({q:text.trim()});
      if(titleId) params.set('titleId',String(titleId));
      if(extId) params.set('extId',extId);
      if(season!==null) {params.set('season',String(season));params.set('min','600');}
      if(episode!==null) params.set('episode',String(episode));
      const result = await searchVideos(params, items => {if(ticket === request)found=items},controller.signal);
      if (ticket === request) found = result;
    } catch (x) {
      if (ticket === request) error = errorText(x);
    } finally {
      if (ticket === request) searching = false;
    }
  }

  /** together - своя приватная комната: смотреть это видео вместе с друзьями (позвать - из комнаты). */
  async function save(url: string, extra: Partial<FoundVideo> = {}, play = false, together = false) {
    busy = true;
    error = '';
    try {
      const r = await post<{ id: number }>('/api/links', {
        url,
        titleId,
        extId,
        season,
        episode,
        label: extra.label ?? null,
        thumb: extra.thumbUrl ?? null,
        duration: extra.duration ?? null,
      });
      onadded?.();
      if (together) {
        const room = await post<{ id: string }>('/api/rooms', { linkId: r.id, position: 0 });
        open = false;
        goto(`/room/${room.id}`);
      } else if (play) {
        open = false;
        goto(`/watch/link/${r.id}`);
      } else if (!extra.url) {
        open = false;
      }
    } catch (x) {
      error = errorText(x);
    } finally {
      busy = false;
    }
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (isUrl) save(text.trim(), {}, true);
    else if (text.trim().length >= 2) search();
  }
</script>

<Modal bind:open title={episode !== null ? `${season} сезон · ${episode} серия` : 'Смотреть по ссылке'} wide>
  {#if saved.length}
    <div class="saved">
      {#each saved.filter((l) => season === null || !l.label.toLowerCase().includes('(фильм')) as link (link.id)}
        <a class="btn" href="/watch/link/{link.id}"><Icon name="play" size={16} />{link.progress && link.progress.position < link.progress.duration * 0.95 ? 'Продолжить' : 'Смотреть'} · {link.label}</a>
      {/each}
    </div>
  {/if}
  <form class="bar" onsubmit={submit}>
    <input class="input" bind:value={text} placeholder="Ссылка (Rutube, VK Видео, YouTube, .mp4) или название" aria-label="Ссылка или название" />
    <button class="btn primary" disabled={busy || text.trim().length < 2}>
      <Icon name={isUrl ? 'play' : 'search'} size={16} /> {isUrl ? 'Смотреть' : 'Найти'}
    </button>
    {#if isUrl}
      <button type="button" class="btn" disabled={busy} onclick={() => save(text.trim(), {}, true, true)}><Icon name="party" size={16} /> Смотреть вместе</button>
    {/if}
  </form>
  <p class="muted hint">Поиск по нескольким источникам. Для сериала проверяем название, сезон и серию; неподходящие фильмы и трейлеры исключаются.</p>
  {#if error}<p class="error">{error}</p>{/if}
  {#if searching && found}<p class="muted small" role="status">Дополняем результаты из других источников…</p>{/if}
  {#if searching && !found}
    <div class="grid">{#each Array(6) as _, i (i)}<div class="ph skeleton"></div>{/each}</div>
  {:else if found}
    {#if found.length}
      <ul class="grid">
        {#each found as f (f.url)}
          <li>
            <button class="hit" onclick={() => save(f.url, f, true)} disabled={busy}>
              <span class="art">
                {#if f.thumb}<img use:fadeIn src={f.thumb} alt="" loading="lazy" decoding="async" />{/if}
                <span class="site">{sourceName(f.site)}</span>
                {#if f.duration}<span class="dur">{clock(f.duration)}</span>{/if}
              </span>
              <span class="name">{f.label}</span>
              {#if f.author}<span class="sub">{f.author}</span>{/if}
            </button>
            <button class="btn sm ghost keep" onclick={() => save(f.url, f)} disabled={busy}><Icon name="plus" size={14} /> Сохранить</button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">{searching ? 'Проверяем источники…' : 'Ничего не нашлось - попробуйте другое название или вставьте ссылку.'}</p>
    {/if}
  {/if}
</Modal>

<style>
  .saved { display: grid; gap: 8px; margin-bottom: 20px; }
  .saved a { height: auto; min-height: 44px; white-space: normal; justify-content: start; padding: 12px 16px; }
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  @media (max-width: 640px) {
    .bar .input {
      flex-basis: 100%;
    }
    .bar .btn {
      flex: 1;
      justify-content: center;
    }
  }
  .bar .input {
    flex: 1;
    min-width: 0;
  }
  .hint {
    font-size: 13px;
    margin: 10px 0 16px;
  }
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 200px), 1fr));
    gap: 16px 14px;
  }
  .ph {
    aspect-ratio: 16 / 9;
    border-radius: 12px;
  }
  li {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .hit {
    display: grid;
    gap: 2px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }
  .art {
    position: relative;
    display: block;
    aspect-ratio: 16 / 9;
    margin-bottom: 6px;
    border-radius: 10px;
    overflow: hidden;
    background: var(--surface);
  }
  .art img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .site,
  .dur {
    position: absolute;
    bottom: 6px;
    padding: 1px 6px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.72);
    font-size: 11px;
    font-weight: 700;
  }
  .site {
    left: 6px;
  }
  .dur {
    right: 6px;
  }
  .name {
    font-size: 13.5px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .keep {
    justify-self: start;
  }
</style>
