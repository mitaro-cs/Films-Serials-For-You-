<script lang="ts">
  import { getDetail, peekDetail } from '$lib/discover/details';
  import { genreRu } from '$lib/genres';
  import { motionOn } from '$lib/motion';
  import Loading from './Loading.svelte';
  import { localized } from '$lib/localized.svelte';
  import type { Card, ExtCard, ExtDetail } from '$lib/types';
  import Icon from './Icon.svelte';

  let { items }: { items: { card?: Card; ext?: ExtCard }[] } = $props();
  let at = $state(0);
  let manual = $state(false);
  let hovering = $state(false);
  let visible = $state(true);
  let root = $state<HTMLElement>();
  let restored = false;
  const selected = $derived(items[Math.min(at, items.length - 1)]);
  let detail = $state<ExtDetail | null>(null);
  const card = $derived(selected?.card ?? detail?.card ?? selected?.ext);
  const href = $derived(selected?.card ? `/title/${selected.card.id}` : `/ext/${selected?.ext?.id}`);
  const backdrop = $derived(selected?.card?.backdrop ?? detail?.backdrop);
  const poster = $derived(selected?.card?.posterLarge ?? detail?.posterLarge ?? card?.poster);
  const overview = $derived(localized(selected?.card?.overview ?? detail?.overview));
  $effect(() => {
    const id = selected?.ext?.id;
    detail = id ? peekDetail(id) : null;
    if (!id) return;
    let live = true;
    let timer = 0;
    let tries = 0;
    const load = () => getDetail(id).then((r) => { if (!live) return; detail = r; if (r.refreshing && ++tries < 12) timer = window.setTimeout(load,3000); }, () => {});
    load();
    return () => { live = false; clearTimeout(timer); };
  });
  $effect(() => {
    if (!root) return;
    const observer = new IntersectionObserver(([entry]) => visible = entry.isIntersecting, { threshold: .2 });
    observer.observe(root); return () => observer.disconnect();
  });
  $effect(() => {
    if (restored || !items.length) return;
    restored = true;
    try { const saved = JSON.parse(sessionStorage.getItem('nova-showcase') ?? 'null'); const i = items.findIndex((c) => String(c.card?.id ?? c.ext?.id) === saved?.id); if (i >= 0) { at = i; manual = !!saved.manual; } } catch { /* пустая сессия */ }
  });
  $effect(() => {
    if (manual || hovering || !visible || items.length < 2 || !motionOn()) return;
    const timer = setInterval(() => { if (document.visibilityState === 'visible') at = (at + 1) % items.length; }, 6500);
    return () => clearInterval(timer);
  });
  $effect(() => {
    if (manual || !visible) return;
    const id = items[(at + 1) % items.length]?.ext?.id;
    if (!id) return;
    const timer = setTimeout(() => { void getDetail(id).catch(() => {}); }, 1600);
    return () => clearTimeout(timer);
  });
  function hold() { manual = true; save(); }
  function save() { try { sessionStorage.setItem('nova-showcase', JSON.stringify({ id: String(selected?.card?.id ?? selected?.ext?.id), manual })); } catch { /* приватный режим */ } }
  function go(delta: number) { manual = true; at = (at + delta + items.length) % items.length; save(); }
</script>

{#if card}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <section class="showcase" aria-label="Рекомендуем" bind:this={root} onpointerenter={() => hovering = true} onpointerleave={() => hovering = false} onpointerdown={hold} onkeydown={hold}>
    {#key backdrop ?? poster}{#if backdrop || poster}<img class="backdrop" class:poster-backdrop={!backdrop} src={backdrop ?? poster ?? ''} alt="" fetchpriority="high" />{/if}{/key}
    <div class="shade"></div>
    {#key card.id}<div class="copy">
      <span class="eyebrow">NOVA · В ЦЕНТРЕ ВНИМАНИЯ</span>
      <h1>{card.title}</h1>
      <p class="meta">
        {#if card.rating}<span class="rating"><Icon name="star" size={16} /> {card.rating.toFixed(1)} <small>IMDb</small></span>{/if}
        {#if card.year}<span>{card.year}</span>{/if}
        <span>{card.kind === 'series' ? 'Сериал' : 'Фильм'}</span>
        {#each card.genres.slice(0, 2) as genre (genre)}<span>{genreRu(genre)}</span>{/each}
      </p>
      {#if overview}<p class="description">{overview}</p>{:else}<p class="description">Ваша следующая история начинается здесь. Откройте подробности, выберите источник и смотрите в Nova.</p>{/if}
      <div class="actions">
        {#if selected.card?.resume}<a class="btn primary" href="/watch/{selected.card.resume.fileId}"><Icon name="play" size={18} /> Продолжить</a>{:else}<a class="btn primary" {href}><Icon name="play" size={18} /> Подробнее</a>{/if}
        <a class="btn" href="/discover">Открыть подборки <Icon name="right" size={16} /></a>
      </div>
      {#if !detail && selected.ext}<Loading compact label="Подробности загружаются в фоне" />{/if}
    </div>{/key}
    {#if poster}<a class="poster" {href} tabindex="-1" aria-hidden="true"><img src={poster} alt="" fetchpriority="high" /></a>{/if}
    {#if items.length > 1}
      <div class="pagination" aria-label="Кадры">
        <button aria-label="Предыдущая рекомендация" onclick={() => go(-1)}><Icon name="back" size={18} /></button>
        <span>{Math.min(at + 1, items.length)} / {items.length}</span>
        <button aria-label="Следующая рекомендация" onclick={() => go(1)}><Icon name="right" size={18} /></button>
        <button onpointerdown={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} aria-label={manual ? 'Включить автоперелистывание' : 'Остановить автоперелистывание'} aria-pressed={!manual} onclick={() => { manual = !manual; save(); }}><Icon name={manual ? 'play' : 'pause'} size={14} /></button>
      </div>
    {/if}
  </section>
{/if}

<style>
  .showcase { position: relative; min-height: 510px; height: min(72svh, 780px); isolation: isolate; display: flex; align-items: center; padding: calc(var(--header) + 36px) var(--gutter) 70px; overflow: hidden; }
  .backdrop, .shade { position: absolute; inset: 0; width: 100%; height: 100%; z-index: -1; }
  .backdrop { object-fit: cover; object-position: center 30%; }
  .shade { background: linear-gradient(90deg, #000e 0%, #0009 46%, #0002), linear-gradient(0deg, #000, transparent 65%); }
  .poster-backdrop { opacity: .22; }
  .copy { animation: feature-in .42s cubic-bezier(.2,.8,.2,1) both; }
  .backdrop { animation: backdrop-in .6s ease both; }
  @keyframes feature-in { from { opacity: 0; transform: translateY(14px); } }
  @keyframes backdrop-in { from { opacity: 0; } }
  .copy { max-width: min(62%, 820px); }
  .eyebrow { font-size: 11px; font-weight: 800; letter-spacing: .16em; color: #ececee; }
  h1 { font-size: clamp(30px, 4.3vw, 72px); line-height: 1.08; letter-spacing: -.035em; margin: 18px 0; overflow-wrap: break-word; }
  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 16px; font-size: 14px; color: #d1d1d6; }
  .rating { display: inline-flex; align-items: center; gap: 5px; color: #f5c518; font-weight: 800; }
  .rating small { color: #aaa; font-weight: 500; }
  .description { max-width: 62ch; font-size: 16px; line-height: 1.6; color: #d1d1d6; display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; margin: 20px 0 24px; }
  .actions { display: flex; gap: 10px; flex-wrap: wrap; }
  .poster { margin-left: auto; width: clamp(150px, 19vw, 290px); flex: none; box-shadow: 0 24px 60px #0008; border-radius: 14px; overflow: hidden; }
  .poster img { display: block; width: 100%; aspect-ratio: 2/3; object-fit: cover; }
  .pagination { position: absolute; bottom: 22px; right: var(--gutter); display: flex; align-items: center; gap: 14px; font-variant-numeric: tabular-nums; font-size: 12px; }
  .pagination button { width: 32px; height: 32px; border-radius: 50%; border: 1px solid #ffffff30; background: #0005; color: white; display: grid; place-items: center; cursor: pointer; }
  @media(max-width: 640px) { .showcase { height: auto; min-height: 500px; padding-top: calc(var(--header) + 90px); padding-bottom: 64px; align-items: flex-end; } .copy { max-width: 100%; } .poster { display: none; } .shade { background: linear-gradient(0deg, var(--bg), #0009 70%, #0005); } .description { font-size: 14px; } .eyebrow { font-size: 10px; } }
  /* «Кинозал»: главная кнопка кадра - белая капсула на любом экране. */
  .actions .btn.primary { background: #fff; color: #0a0a0a; border: 0; height: 46px; padding: 0 26px; border-radius: 999px; font-weight: 700; transition: transform .15s ease; }
  .actions .btn.primary:active { transform: scale(.96); }
  /* Телефон - «Кинозал»: кадр почти на весь экран, внизу сведения, название и одна белая «Смотреть». */
  @media (max-width: 640px) {
    .showcase { min-height: 0; height: clamp(460px, 74svh, 640px); padding-bottom: 54px; }
    .eyebrow, .description, .actions .btn:not(.primary) { display: none; }
    .copy h1 { font-size: clamp(32px, 10vw, 44px); letter-spacing: -.035em; margin: 6px 0 12px; }
    .meta { font-size: 12px; text-transform: uppercase; letter-spacing: .06em; font-weight: 600; gap: 4px 10px; }
    .pagination { bottom: 14px; }
  }
</style>
