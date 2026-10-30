<script lang="ts">
  import { page } from '$app/state';
  import { api } from '$lib/api';
  import { getDetail } from '$lib/discover/details';
  import { artW, placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import Loading from '$lib/ui/Loading.svelte';
  import type { ExtCard, ExtDetail } from '$lib/types';
  type Group = { name: string; items: ExtCard[] };
  const id = $derived(page.params.id ?? '');
  let groups = $state<Group[] | null>(null), error = $state(''), chosen = $state('');
  let detail = $state<ExtDetail | null>(null);
  const group = $derived(groups?.[Math.max(0, Math.min(groups.length - 1, Number(page.url.searchParams.get('group')) || 0))] ?? null);
  const current = $derived(group?.items.find(x => x.id === chosen) ?? group?.items[0] ?? null);
  $effect(() => {
    const want = id; groups = null; error = ''; chosen = want;
    api<Group[]>(`/api/franchise?imdb=${encodeURIComponent(want)}`).then(g => { if (id === want) groups = g.filter(x => x.items.length > 1); }, () => { if (id === want) error = 'Не удалось загрузить подборку.'; });
  });
  $effect(() => {
    const want = current?.id; detail = null; if (!want) return;
    let live = true, timer = 0, attempts = 0;
    const load = () => getDetail(want).then(d => { if (!live) return; detail = d; if (d.refreshing && attempts++ < 8) timer = window.setTimeout(load,2500); }, () => {});
    load(); return () => { live = false; clearTimeout(timer); };
  });
</script>
{#if error}<div class="message"><p>{error}</p><a class="btn" href="/ext/{id}">Вернуться к фильму</a></div>
{:else if groups === null}<div class="message"><Loading label="Собираем фильмы серии" /></div>
{:else if !group || !current}<div class="message"><p>Подборка для этого фильма пока не найдена.</p><a class="btn" href="/ext/{id}">Вернуться к фильму</a></div>
{:else}
  <main class="collection">
    <div class="backdrop" style:background={placeholder(current.title)} aria-hidden="true">{#if detail?.backdrop}<img src={artW(detail.backdrop,1920)} alt="" />{/if}</div>
    <div class="shade"></div>
    <section class="content" aria-label="Фильмы серии">
      <a class="back" href="/ext/{id}"><Icon name="back" size={16} /> К фильму</a>
      <h1>{group.name}</h1><p class="sub">Все фильмы серии</p>
      <ol>
        {#each group.items as film, i (film.id)}
          <li><a href={film.local ? `/title/${film.local}` : `/ext/${film.id}`} class:active={film.id === current.id} onmouseenter={() => chosen = film.id} onfocus={() => chosen = film.id}>
            <span class="art" style:background={placeholder(film.title)}>{#if film.poster}<img src={artW(film.poster,640)} alt="" loading="lazy" />{/if}</span>
            <span class="label"><b>{film.title}</b><small>{film.year ?? ''}</small></span><span class="num">{String(i+1).padStart(2,'0')}</span>
          </a></li>
        {/each}
      </ol>
    </section>
  </main>
{/if}
<style>
  .message{min-height:60vh;display:grid;place-content:center;text-align:center;gap:20px;padding:24px}
  .collection{position:relative;min-height:100svh;background:#303a3a;color:#fff;isolation:isolate}
  .backdrop{position:absolute;inset:0 43% 0 0;overflow:hidden;z-index:-2}.backdrop img{position:sticky;top:0;width:100%;max-width:none;height:100svh;object-fit:cover;object-position:center 25%;animation:appear .4s ease}
  .shade{position:absolute;inset:0 40% 0 36%;background:linear-gradient(90deg,transparent,#303a3a);z-index:-1;pointer-events:none}
  .content{width:46%;min-height:100svh;margin-left:auto;padding:calc(var(--header) + 30px) clamp(20px,3vw,50px) 35px;background:#303a3a}
  .back{display:inline-flex;align-items:center;gap:6px;color:#d4d9d9;font-size:13px}h1{font-size:clamp(28px,3vw,46px);margin:20px 0 4px;line-height:1.1}.sub{color:#c0caca;margin:0 0 18px;font-size:13px}
  ol{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:9px}li{min-width:0}li a{display:grid;grid-template-columns:54px minmax(0,1fr) 23px;gap:10px;align-items:center;min-width:0;padding:7px;background:#ffffff0d;border:1px solid transparent;border-radius:10px;transition:background .2s,border-color .2s,transform .2s}li a:hover,li a:focus-visible,li a.active{background:#ffffff20;border-color:#ffffff7a}.art{aspect-ratio:2/3;overflow:hidden;border-radius:5px}.art img{width:100%;height:100%;object-fit:cover}.label{display:grid;gap:4px;min-width:0}.label b{font-size:12px;line-height:1.3;overflow-wrap:anywhere}.label small,.num{font-size:11px;color:#cbd3d3}.num{text-align:right}
  @keyframes appear{from{opacity:0}}@media(prefers-reduced-motion:reduce){*{animation:none!important;transition:none!important}}
  @media(max-width:760px){.backdrop{inset:0 0 auto;height:38svh}.backdrop img{height:100%;position:static}.shade{inset:0 0 auto;height:38svh;background:linear-gradient(0deg,#303a3a,transparent 70%)}.content{width:100%;padding:32svh 16px 90px;background:transparent}ol{gap:8px}li a{grid-template-columns:46px minmax(0,1fr);gap:8px}.num{display:none}.label b{font-size:11px}}
</style>
