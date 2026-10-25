<script lang="ts">
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';
  const genres: { label: string; chart: string; icon: IconName }[] = [
    {label:'Комедии',chart:'comedy_movies',icon:'sparkle'}, {label:'Фантастика',chart:'scifi_movies',icon:'globe'},
    {label:'Триллеры',chart:'thriller_movies',icon:'eye'}, {label:'Боевики',chart:'action_movies',icon:'play'},
    {label:'Драмы',chart:'drama_movies',icon:'film'}, {label:'Анимация',chart:'animation_movies',icon:'star'},
  ];
  const franchises = ['Гарри Поттер','Властелин колец','Звёздные войны','Матрица','Мстители'];
</script>
<section class="browse" aria-label="Жанры"><h2>Под настроение</h2><div class="genres">{#each genres as genre}<a href="/discover?chart={genre.chart}"><Icon name={genre.icon} size={27} /><b>{genre.label}</b><span>Открыть подборку <Icon name="right" size={13} /></span></a>{/each}</div></section>
<section class="browse" aria-label="Франшизы"><h2>Любимые киновселенные</h2><div class="franchises">{#each franchises as title}<a href="/search?q={encodeURIComponent(title)}"><Icon name="film" size={18} />{title}<Icon name="right" size={14} /></a>{/each}</div></section>
<style>
  .browse{margin:30px 0 38px}h2{font-size:clamp(19px,1.7vw,25px);margin:0 0 18px}.genres{display:grid;grid-template-columns:repeat(6,minmax(0,1fr));gap:12px}.genres a{min-width:0;display:flex;flex-direction:column;gap:18px;justify-content:space-between;padding:24px 18px;border:1px solid var(--line);background:#000;border-radius:16px;transition:transform .2s,border-color .2s}.genres a:hover{transform:translateY(-4px);border-color:#fff8}.genres b{font-size:clamp(15px,1.8vw,23px)}.genres span{display:flex;align-items:center;gap:6px;color:var(--muted);font-size:10px}.franchises{display:flex;gap:10px;overflow-x:auto;padding-bottom:8px}.franchises a{display:flex;align-items:center;gap:9px;flex:none;padding:16px 20px;border:1px solid var(--line);border-radius:12px;background:#000;font-size:13px}.franchises a:hover{border-color:#fff8}@media(max-width:1000px){.genres{grid-template-columns:repeat(3,minmax(0,1fr))}}@media(max-width:600px){.genres{grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}.genres a{padding:18px 14px;gap:14px}.franchises a{padding:14px}}
</style>
