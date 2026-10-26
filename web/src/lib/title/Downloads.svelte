<script lang="ts">
  import { fadeIn } from '$lib/ui/fade';
  import { count, size } from '$lib/fmt';
  import type { FileItem, Kind } from '$lib/types';
  import { artW, placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';

  // Скачать: сериал - целиком или по сезонам одним архивом (ZIP без сжатия, размер виден сразу),
  // серии - по одной; фильм - файлом (из нескольких частей - ещё и архивом).
  let { titleId, title, kind, files }: { titleId: number; title: string; kind: Kind; files: FileItem[] } = $props();

  const total = $derived(files.reduce((a, f) => a + f.size, 0));
  const seasons = $derived.by(() => {
    const m = new Map<number, FileItem[]>();
    for (const f of files) m.set(f.season ?? 1, [...(m.get(f.season ?? 1) ?? []), f]);
    return [...m.entries()].sort((a, b) => a[0] - b[0]).map(([season, items]) => ({ season, items, bytes: items.reduce((a, f) => a + f.size, 0) }));
  });
  let open = $state<number | null>(null);
  const code = (f: FileItem) => (f.season !== null && f.episode !== null ? `${f.season}×${String(f.episode).padStart(2, '0')}` : '');
</script>

<div class="dl">
  {#if kind === 'series'}
    <div class="all">
      <div class="sum">
        <b>{title}</b>
        <span class="muted">{count(seasons.length, 'сезон', 'сезона', 'сезонов')} · {count(files.length, 'серия', 'серии', 'серий')} · {size(total)}</span>
      </div>
      <a class="btn white" href="/api/titles/{titleId}/download" download><Icon name="download" size={16} /> Скачать весь сериал</a>
    </div>
    <ul class="seasons">
      {#each seasons as s, i (s.season)}
        <li class="season rise" style:--i={i}>
          <div class="head">
            <button class="toggle" aria-expanded={open === s.season} onclick={() => (open = open === s.season ? null : s.season)}>
              <span class="num">{s.season}</span>
              <span class="what">
                <b>{s.season} сезон</b>
                <small class="muted">{count(s.items.length, 'серия', 'серии', 'серий')} · {size(s.bytes)}</small>
              </span>
              <span class="chev" class:up={open === s.season}><Icon name="down" size={18} /></span>
            </button>
            <a class="btn sm" href="/api/titles/{titleId}/download?season={s.season}" download aria-label="Скачать {s.season} сезон архивом">
              <Icon name="download" size={14} /> Сезон
            </a>
          </div>
          {#if open === s.season}
            <ol class="eps">
              {#each s.items as f, k (f.id)}
                <li style:--k={k}>
                  <span class="still" style:background={f.still ? undefined : placeholder(title + f.id)}>
                    {#if f.still}<img use:fadeIn src={artW(f.still, 342)} alt="" loading="lazy" decoding="async" width="96" height="54" />{/if}
                  </span>
                  <span class="name">
                    <b>{code(f)}</b>
                    <span>{f.name ?? f.fileName}</span>
                  </span>
                  <small class="muted size">{size(f.size)}</small>
                  <a class="get" href="/api/files/{f.id}/download" download aria-label="Скачать {code(f)} {f.name ?? f.fileName}" title="Скачать серию">
                    <Icon name="download" size={16} />
                  </a>
                </li>
              {/each}
            </ol>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <ul class="files">
      {#each files as f (f.id)}
        <li class="file">
          <span class="ic"><Icon name="film" size={20} /></span>
          <span class="name">
            <b>{f.fileName}</b>
            <small class="muted">{size(f.size)}{f.duration ? ` · ${Math.round(f.duration / 60)} мин` : ''}</small>
          </span>
          <a class="btn white sm" href="/api/files/{f.id}/download" download><Icon name="download" size={14} /> Скачать</a>
        </li>
      {/each}
    </ul>
    {#if files.length > 1}
      <a class="btn sm" href="/api/titles/{titleId}/download" download><Icon name="download" size={14} /> Все части одним архивом · {size(total)}</a>
    {/if}
  {/if}
</div>

<style>
  .dl {
    display: grid;
    gap: 14px;
  }
  .all {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px 20px;
    padding: 16px 18px;
    border-radius: 16px;
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.09), rgba(255, 255, 255, 0.03));
    border: 1px solid var(--line);
  }
  .sum {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .sum b {
    font-size: 16px;
    overflow-wrap: anywhere;
  }
  .sum span {
    font-size: 13px;
  }
  .seasons,
  .eps,
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  .season {
    border-radius: 14px;
    background: #232325;
    overflow: hidden;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px 8px 8px;
  }
  .toggle {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 4px;
    border: 0;
    border-radius: 10px;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .num {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    flex: none;
    border-radius: 11px;
    background: rgba(255, 255, 255, 0.1);
    font-weight: 800;
    font-size: 16px;
  }
  .what {
    display: grid;
    min-width: 0;
  }
  .what small {
    font-size: 12.5px;
  }
  .chev {
    margin-left: auto;
    display: grid;
    color: var(--muted);
    transition: transform 0.3s cubic-bezier(0.2, 0.9, 0.3, 1.3);
  }
  .chev.up {
    transform: rotate(180deg);
  }
  .eps {
    gap: 2px;
    padding: 0 8px 8px;
  }
  .eps li,
  .file {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    padding: 6px;
    border-radius: 10px;
    animation: in 0.35s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(min(var(--k, 0), 14) * 25ms);
    transition: background 0.15s;
  }
  .eps li:hover {
    background: rgba(255, 255, 255, 0.05);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
  .still {
    position: relative;
    display: block;
    width: 72px;
    aspect-ratio: 16 / 9;
    border-radius: 6px;
    overflow: hidden;
  }
  .still img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .name {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    font-size: 14px;
  }
  .name span,
  .name b {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name b {
    flex: none;
    font-variant-numeric: tabular-nums;
  }
  .size {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .get {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    transition:
      background 0.15s,
      transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.5);
  }
  .get:hover {
    background: #fff;
    color: #000;
    transform: translateY(-2px);
  }
  .file {
    grid-template-columns: auto minmax(0, 1fr) auto;
    padding: 12px;
    background: #232325;
  }
  .file .name {
    display: grid;
    gap: 2px;
  }
  .file .name b {
    flex: initial;
  }
  .ic {
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border-radius: 11px;
    background: rgba(255, 255, 255, 0.1);
  }
  @media (max-width: 560px) {
    .still {
      display: none;
    }
    .eps li {
      grid-template-columns: minmax(0, 1fr) auto auto;
    }
    .name {
      display: grid;
      gap: 0;
    }
  }
</style>
