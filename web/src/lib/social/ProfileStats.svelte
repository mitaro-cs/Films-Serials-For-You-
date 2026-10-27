<script lang="ts">
  import { count, plural } from '$lib/fmt';
  import type { ProfileStats } from '$lib/types';

  // Статистика на личной странице: плитки, любимые жанры и досмотренное по месяцам.
  let { stats, accent = null }: { stats: ProfileStats; accent?: string | null } = $props();

  const MONTHS = ['янв', 'фев', 'мар', 'апр', 'май', 'июн', 'июл', 'авг', 'сен', 'окт', 'ноя', 'дек'];
  // months[11] - текущий месяц, левее - раньше (по 30 дней).
  const now = new Date().getMonth();
  const label = (i: number) => MONTHS[(now - (11 - i) + 24) % 12];
  const fullLabel = (i: number) => stats.monthLabels?.[i] ? new Date(`${stats.monthLabels[i]}-01T00:00:00Z`).toLocaleDateString('ru-RU', { month: 'long', year: 'numeric', timeZone: 'UTC' }) : label(i);
  let selected = $state<number | null>(null);
  const total = $derived(stats.months.reduce((a, b) => a + b, 0));
  const active = $derived(stats.months.filter((m) => m > 0).length);
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
      <div class="chart-title"><h3>Досмотрено за год</h3><b>{total} <small>фильмов и серий</small></b></div>
      <div class="chart-summary"><span>{count(active, 'активный месяц', 'активных месяца', 'активных месяцев')}</span><span>В среднем {(total / 12).toLocaleString('ru-RU', { maximumFractionDigits: 1 })} в месяц</span></div>
      <div class="chart-area"><div class="axis"><span>{peak}</span><span>{Math.round(peak / 2)}</span><span>0</span></div><div class="months">
        {#each stats.months as m, i (i)}
          <button class="col" class:selected={selected === i} title="{fullLabel(i)}: {m}" aria-label="{fullLabel(i)}: досмотрено {m}" aria-pressed={selected === i} onclick={() => selected = i}>
            <span class="value">{m}</span>
            <span class="plot"><span class="bar" class:zero={!m} style:height="{m ? Math.max(4, m / peak * 100) : 2}%"></span></span>
            <small>{label(i)}</small>
          </button>
        {/each}
      </div></div>
      <p class="chart-detail" aria-live="polite">{selected === null ? 'Выберите месяц, чтобы посмотреть число завершённых фильмов и серий.' : `${fullLabel(selected)} · досмотрено ${stats.months[selected]}`}</p>
      {#if !total}<p class="muted">Пока нет завершённых просмотров. Начатые фильмы учитываются в часах просмотра.</p>{/if}
      <p class="chart-note">Часы рассчитаны по сохранённым позициям локального видео и внешних источников. Отметка «Просмотрел» учитывается в количестве, но не добавляет выдуманное время.</p>
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
    height: 180px;
    flex: 1;
    background: repeating-linear-gradient(to top, transparent 0, transparent 49px, #ffffff0c 50px);
  }
  .col {
    display: grid;
    grid-template-rows: 18px minmax(0,1fr) 18px;
    justify-items: center;
    gap: 6px;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    cursor: pointer;
  }
  .plot { display: flex; align-items: flex-end; width: 100%; justify-content: center; height: 100%; }
  .value { font-size: 11px; color: var(--muted); font-variant-numeric: tabular-nums; }
  .col.selected .bar { background: #f5c518; }
  .chart-area { display: flex; gap: 10px; }
  .axis { display: flex; flex-direction: column; justify-content: space-between; padding: 22px 0 22px; font-size: 10px; color: var(--muted); width: 22px; }
  .chart-title { display: flex; justify-content: space-between; gap: 16px; align-items: baseline; }
  .chart-title b { font-size: 24px; white-space: nowrap; }
  .chart-title b small { font-size: 11px; color: var(--muted); font-weight: 400; }
  .chart-summary { display: flex; justify-content: space-between; gap: 12px; color: var(--muted); font-size: 11px; margin-bottom: 18px; }
  .chart-detail { font-size: 12px; margin: 16px 0 10px; }
  .chart-note { font-size: 11px; color: var(--muted); line-height: 1.6; margin: 0; }
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
  /* Узкий телефон: двенадцать подписей не помещаются - через одну (выбранный месяц - всегда). */
  @media (max-width: 480px) {
    .months {
      gap: 3px;
    }
    .col:nth-child(odd):not(.selected) small {
      visibility: hidden;
    }
    .col small,
    .value {
      font-size: 9.5px;
    }
  }
</style>
