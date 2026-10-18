<script lang="ts">
  import { count, plural } from '$lib/fmt';
  import type { ProfileStats } from '$lib/types';

  // Статистика на личной странице: плитки, любимые жанры и досмотренное по месяцам.
  let { stats, accent = null }: { stats: ProfileStats; accent?: string | null } = $props();

  const MONTHS = ['янв', 'фев', 'мар', 'апр', 'май', 'июн', 'июл', 'авг', 'сен', 'окт', 'ноя', 'дек'];
  // months[11] — текущий месяц, левее — раньше (по 30 дней).
  const now = new Date().getMonth();
  const label = (i: number) => MONTHS[(now - (11 - i) + 24) % 12];
  const peak = $derived(Math.max(1, ...stats.months));
  const top = $derived(Math.max(1, ...stats.genres.map((g) => g.count)));
  const hours = $derived(stats.hours >= 10 ? Math.round(stats.hours) : stats.hours);

  const tiles = $derived([
    { n: hours, label: `${plural(Math.floor(stats.hours), 'час', 'часа', 'часов')} просмотра` },
    { n: stats.movies, label: plural(stats.movies, 'фильм', 'фильма', 'фильмов') },
    { n: stats.episodes, label: `${plural(stats.episodes, 'серия', 'серии', 'серий')} · ${count(stats.series, 'сериал', 'сериала', 'сериалов')}` },
    { n: stats.outside, label: 'отмечено в Обзоре' },
    { n: stats.reviews, label: plural(stats.reviews, 'отзыв', 'отзыва', 'отзывов') },
    { n: stats.friends, label: plural(stats.friends, 'друг', 'друга', 'друзей') },
  ]);
</script>

<div class="stats" style:--acc={accent ?? '#f5f5f7'}>
  <ul class="tiles">
    {#each tiles as t (t.label)}
      <li><b>{t.n}</b><span>{t.label}</span></li>
    {/each}
  </ul>

  <div class="charts">
    <section class="card" aria-label="Любимые жанры">
      <h3>Любимые жанры</h3>
      {#if stats.genres.length}
        <ul class="bars">
          {#each stats.genres as g (g.genre)}
            <li>
              <span class="g">{g.genre}</span>
              <span class="track"><i style:transform="scaleX({g.count / top})"></i></span>
              <span class="n">{g.count}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted">Появятся, когда что-нибудь посмотрите.</p>
      {/if}
    </section>
    <section class="card" aria-label="По месяцам">
      <h3>Досмотрено за год</h3>
      <div class="months">
        {#each stats.months as m, i (i)}
          <div class="col" title="{label(i)}: {m}">
            <span class="bar" class:zero={!m} style:transform="scaleY({m ? Math.max(0.06, m / peak) : 0.03})"></span>
            <small>{label(i)}</small>
          </div>
        {/each}
      </div>
    </section>
  </div>
</div>

<style>
  .stats {
    display: grid;
    gap: 16px;
  }
  .tiles {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 10px;
  }
  @media (max-width: 900px) {
    .tiles {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
  .tiles li {
    display: grid;
    gap: 2px;
    padding: 14px 16px;
    border-radius: var(--radius);
    background: var(--surface);
    min-width: 0;
  }
  .tiles b {
    font-size: 28px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--acc);
    font-variant-numeric: tabular-nums;
  }
  .tiles span {
    font-size: 13px;
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  .charts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 300px), 1fr));
    gap: 16px;
  }
  .charts .card {
    padding: 18px;
    background: var(--surface);
    border-radius: var(--radius);
    min-width: 0;
  }
  h3 {
    font-size: 15px;
    font-weight: 600;
    margin: 0 0 14px;
  }
  .bars {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .bars li {
    display: grid;
    grid-template-columns: minmax(0, 110px) minmax(0, 1fr) 28px;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .g {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-transform: capitalize;
  }
  .track {
    height: 8px;
    border-radius: 99px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .track i {
    display: block;
    height: 100%;
    background: var(--acc);
    border-radius: 99px;
    transform-origin: left;
  }
  .n {
    text-align: right;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .months {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 6px;
    height: 150px;
  }
  .col {
    display: grid;
    grid-template-rows: 1fr auto;
    justify-items: center;
    gap: 6px;
    min-width: 0;
  }
  .bar {
    align-self: end;
    width: 100%;
    max-width: 22px;
    height: 100%;
    border-radius: 6px;
    background: var(--acc);
    transform-origin: bottom;
  }
  .bar.zero {
    background: var(--surface-2);
  }
  .col small {
    font-size: 10.5px;
    color: var(--faint);
  }
</style>
