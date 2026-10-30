<script lang="ts">
  import { page } from '$app/state';
  import { fade } from 'svelte/transition';
  import { api } from '$lib/api';
  import { getDetail } from '$lib/discover/details';
  import { artSet, artW, placeholder } from '$lib/ui/art';
  import { localized } from '$lib/localized.svelte';
  import { toneOfUrl } from '$lib/ui/tone';
  import Icon from '$lib/ui/Icon.svelte';
  import Loading from '$lib/ui/Loading.svelte';
  import type { ExtCard, ExtDetail } from '$lib/types';

  // Серия фильмов: слева кадр выбранного фильма (цвет страницы - по кадру, плавно меняется), под ним
  // постер, год, оценка и описание; справа - все фильмы серии. Наведёшь на фильм - слева он же.
  type Group = { name: string; items: ExtCard[] };
  const id = $derived(page.params.id ?? '');
  let groups = $state<Group[] | null>(null), error = $state(''), chosen = $state('');
  let detail = $state<ExtDetail | null>(null);
  let tone = $state('#2a3434');
  const group = $derived(groups?.[Math.max(0, Math.min(groups.length - 1, Number(page.url.searchParams.get('group')) || 0))] ?? null);
  const current = $derived(group?.items.find(x => x.id === chosen) ?? group?.items[0] ?? null);
  const href = (f: ExtCard) => (f.local ? `/title/${f.local}` : `/ext/${f.id}`);
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
  // Цвет страницы - по кадру выбранного фильма (нет кадра - по постеру).
  $effect(() => {
    const src = detail?.backdrop ? artW(detail.backdrop, 640) : current?.poster ? artW(current.poster, 342) : null;
    if (!src) return;
    let live = true;
    void toneOfUrl(src).then(c => { if (live && c) tone = c; });
    return () => { live = false; };
  });
</script>
{#if error}<div class="message"><p>{error}</p><a class="btn" href="/ext/{id}">Вернуться к фильму</a></div>
{:else if groups === null}<div class="message"><Loading label="Собираем фильмы серии" /></div>
{:else if !group || !current}<div class="message"><p>Подборка для этого фильма пока не найдена.</p><a class="btn" href="/ext/{id}">Вернуться к фильму</a></div>
{:else}
  <main class="collection" style:--tone={tone}>
    <div class="stage" aria-hidden="true">
      <div class="frame" style:background={placeholder(current.title)}>
        {#key detail?.backdrop}{#if detail?.backdrop}<img in:fade|global={{ duration: 600 }} out:fade|global={{ duration: 600 }} src={artW(detail.backdrop, 1920)} srcset={artSet(detail.backdrop)} sizes="54vw" alt="" decoding="async" />{/if}{/key}
      </div>
      <div class="veil"></div>
      {#key current.id}
        <div class="film" in:fade|global={{ duration: 450 }}>
          <span class="poster" style:background={placeholder(current.title)}>{#if current.poster}<img src={artW(detail?.posterLarge ?? current.poster, 640)} alt="" decoding="async" />{/if}</span>
          <div class="text">
            <b class="name">{current.title}</b>
            <span class="meta">{[current.year, current.rating ? `★ ${current.rating.toFixed(1)}` : ''].filter(Boolean).join('  ·  ')}</span>
            {#if detail?.overview}<p class="ov">{localized(detail.overview, '')}</p>{/if}
          </div>
        </div>
      {/key}
    </div>
    <section class="content" aria-label="Фильмы серии">
      <a class="back" href="/ext/{id}"><Icon name="back" size={16} /> К фильму</a>
      <h1>{group.name}</h1><p class="sub">Все фильмы серии</p>
      <ol>
        {#each group.items as film, i (film.id)}
          <li><a href={href(film)} class:active={film.id === current.id} onmouseenter={() => chosen = film.id} onfocus={() => chosen = film.id}>
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
  .collection{position:relative;display:grid;grid-template-columns:54% minmax(0,1fr);min-height:100svh;color:#fff;background:color-mix(in srgb,var(--tone) 55%,#000);transition:background-color .8s ease;isolation:isolate}
  .stage{position:sticky;top:0;height:100svh;overflow:hidden}
  .frame{position:absolute;inset:0 0 34% 0}
  .frame img{position:absolute;inset:0;width:100%;height:100%;max-width:none;object-fit:cover;object-position:center 25%}
  .veil{position:absolute;inset:0;background:linear-gradient(0deg,color-mix(in srgb,var(--tone) 55%,#000) 30%,transparent 70%),linear-gradient(270deg,color-mix(in srgb,var(--tone) 55%,#000) 0%,transparent 40%),linear-gradient(180deg,#0000008c,transparent 22%)}
  .film{position:absolute;left:0;right:0;bottom:0;display:flex;align-items:flex-end;gap:22px;padding:0 clamp(24px,3vw,48px) clamp(28px,5vh,56px)}
  .poster{flex:none;width:clamp(110px,10vw,160px);aspect-ratio:2/3;border-radius:12px;overflow:hidden;box-shadow:0 24px 50px -18px #000c}
  .poster img{width:100%;height:100%;object-fit:cover}
  .text{display:grid;gap:8px;min-width:0}
  .name{font-size:clamp(22px,2.4vw,36px);line-height:1.1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;padding-bottom:.08em}
  .meta{color:#e0e6e6;font-size:14px}
  .ov{margin:0;color:#d6dcdc;font-size:14px;line-height:1.55;display:-webkit-box;-webkit-line-clamp:3;line-clamp:3;-webkit-box-orient:vertical;overflow:hidden;max-width:60ch}
  .content{min-width:0;min-height:100svh;padding:calc(var(--header) + 30px) clamp(20px,3vw,50px) 35px 0}
  .back{display:inline-flex;align-items:center;gap:6px;color:#d4d9d9;font-size:13px}h1{font-size:clamp(28px,3vw,46px);margin:20px 0 4px;line-height:1.1}.sub{color:#c0caca;margin:0 0 18px;font-size:13px}
  ol{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:9px}li{min-width:0}li a{display:grid;grid-template-columns:54px minmax(0,1fr) 23px;gap:10px;align-items:center;min-width:0;padding:7px;background:#ffffff0d;border:1px solid transparent;border-radius:10px;transition:background .2s,border-color .2s,transform .2s}li a:hover,li a:focus-visible,li a.active{background:#ffffff20;border-color:#ffffff7a}.art{aspect-ratio:2/3;overflow:hidden;border-radius:5px}.art img{width:100%;height:100%;object-fit:cover}.label{display:grid;gap:4px;min-width:0}.label b{font-size:12px;line-height:1.3;overflow-wrap:anywhere}.label small,.num{font-size:11px;color:#cbd3d3}.num{text-align:right}
  @media(max-width:760px){.collection{display:block}.stage{position:relative;height:56svh}.frame{inset:0 0 30% 0}.film{padding:0 16px 16px}.poster{width:84px}.ov{display:none}.content{padding:8px 16px 90px}ol{gap:8px}li a{grid-template-columns:46px minmax(0,1fr);gap:8px}.num{display:none}.label b{font-size:11px}}
</style>
