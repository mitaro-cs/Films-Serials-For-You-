<script lang="ts">
  import { api } from '$lib/api';

  // Оценки серий как на SeriesGraph: сезоны - столбцами, серии - строками, цвет - по оценке IMDb.
  // Грузится, когда раздел показался на экране (первый раз сервер берёт сезоны с IMDb).
  let { imdbId }: { imdbId: string } = $props();

  type Season = { season: number; episodes: { episode: number; rating: number | null; name: string | null }[] };
  let data = $state<Season[] | null>(null);
  let failed = $state(false);
  let box = $state<HTMLElement>();

  $effect(() => {
    const id = imdbId;
    if (!box) return;
    data = null;
    failed = false;
    const io = new IntersectionObserver(
      (e) => {
        if (!e[0].isIntersecting) return;
        io.disconnect();
        api<{ seasons: Season[] }>(`/api/discover/title/${id}/ratings`).then(
          (r) => id === imdbId && (data = r.seasons),
          () => (failed = true),
        );
      },
      { rootMargin: '300px 0px' },
    );
    io.observe(box);
    return () => io.disconnect();
  });

  const LEGEND = [
    { cls: 'awesome', label: 'Шедевр', min: 9 },
    { cls: 'great', label: 'Отлично', min: 8 },
    { cls: 'good', label: 'Хорошо', min: 7 },
    { cls: 'regular', label: 'Средне', min: 6 },
    { cls: 'bad', label: 'Плохо', min: 5 },
    { cls: 'garbage', label: 'Провал', min: 0 },
  ];
  const cls = (r: number | null) => (r === null ? 'none' : (LEGEND.find((l) => r >= l.min)?.cls ?? 'garbage'));
  const rows = $derived(data ? Math.max(0, ...data.map((s) => Math.max(0, ...s.episodes.map((e) => e.episode)))) : 0);
  const cell = (s: Season, ep: number) => s.episodes.find((e) => e.episode === ep);
  const best = $derived.by(() => {
    let top: { s: number; e: number; r: number } | null = null;
    for (const s of data ?? []) for (const e of s.episodes) if (e.rating && (!top || e.rating > top.r)) top = { s: s.season, e: e.episode, r: e.rating };
    return top;
  });
</script>

<div class="rg" bind:this={box}>
  {#if failed}
    <p class="muted">IMDb сейчас не отвечает - оценки серий появятся позже.</p>
  {:else if data === null}
    <div class="sk skeleton" aria-busy="true"></div>
  {:else if !data.length}
    <p class="muted">У IMDb нет оценок серий этого сериала.</p>
  {:else}
    <ul class="legend" aria-label="Цвета оценок">
      {#each LEGEND as l (l.cls)}<li><i class={l.cls}></i>{l.label}</li>{/each}
    </ul>
    {#if best}<p class="best">Лучшая серия - <b>{best.s}×{String(best.e).padStart(2, '0')}</b> · {best.r.toFixed(1)}</p>{/if}
    <div class="scroll">
      <table style:--cols={data.length}>
        <thead>
          <tr><th></th>{#each data as s (s.season)}<th scope="col">S{s.season}</th>{/each}</tr>
        </thead>
        <tbody>
          {#each Array(rows) as _, k (k)}
            <tr>
              <th scope="row">E{k + 1}</th>
              {#each data as s, j (s.season)}
                {@const c = cell(s, k + 1)}
                <td>
                  {#if c}
                    <span class="c {cls(c.rating)}" style:--d="{Math.min(j + k, 40) * 18}ms" title="{s.season}×{String(k + 1).padStart(2, '0')}{c.name ? ` · ${c.name}` : ''}">
                      {c.rating ? c.rating.toFixed(1) : '-'}
                    </span>
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .rg {
    min-width: 0;
  }
  .sk {
    height: 220px;
    border-radius: 14px;
  }
  .legend {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    font-size: 13px;
    color: var(--muted);
  }
  .legend li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .legend i {
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }
  .best {
    margin: 0 0 14px;
    font-size: 13.5px;
    color: var(--muted);
  }
  .best b {
    color: var(--text);
  }
  .scroll {
    overflow-x: auto;
    padding-bottom: 4px;
  }
  table {
    border-collapse: separate;
    border-spacing: 5px;
    margin: -5px;
  }
  th {
    font-size: 12.5px;
    font-weight: 700;
    color: var(--muted);
    text-align: center;
    padding: 0 4px;
  }
  tbody th {
    text-align: right;
    min-width: 34px;
  }
  td {
    padding: 0;
  }
  .c {
    display: grid;
    place-items: center;
    width: 54px;
    height: 32px;
    border-radius: 7px;
    font-size: 14px;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
    cursor: default;
    animation: pop 0.45s cubic-bezier(0.2, 0.9, 0.3, 1.4) both;
    animation-delay: var(--d);
    transition: transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.5);
  }
  .c:hover {
    filter: brightness(1.12);
    box-shadow: inset 0 0 0 2px rgba(255, 255, 255, 0.8);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.6);
    }
  }
  /* Цвета как у SeriesGraph: тёмно-зелёный - шедевр … фиолетовый - провал. */
  .awesome {
    background: #1d6b3a;
    color: #fff;
  }
  .great {
    background: #2fa65a;
    color: #06200f;
  }
  .good {
    background: #f2c94c;
    color: #2b2200;
  }
  .regular {
    background: #f2994a;
    color: #2b1400;
  }
  .bad {
    background: #e5484d;
    color: #fff;
  }
  .garbage {
    background: #7c4dd8;
    color: #fff;
  }
  .none {
    background: rgba(255, 255, 255, 0.08);
    color: var(--faint);
  }
  @media (max-width: 640px) {
    .c {
      width: 46px;
      height: 30px;
      font-size: 13px;
    }
  }

  .rg { display: grid; grid-template-columns: minmax(150px, .24fr) minmax(0, 1fr); gap: 16px 30px; align-items: start; }
  .legend { display: grid; grid-column: 1; gap: 10px; }
  .best { grid-column: 1; padding: 16px; background: var(--surface-2); border-radius: 14px; }
  .scroll { grid-column: 2; grid-row: 1 / span 2; }
  table { width: 100%; table-layout: fixed; margin: 0; min-width: calc(var(--cols) * 50px + 36px); }
  thead th:first-child { width: 36px; }
  .c { width: 100%; min-width: 40px; height: 36px; }
  @media(max-width: 700px) { .rg { display: block; } .legend { display: flex; } .best { padding: 10px; } }

</style>
