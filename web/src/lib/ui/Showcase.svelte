<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { artW, heroSet } from '$lib/ui/art';
  import { toneOfUrl } from '$lib/ui/tone';
  import { fitTitle } from '$lib/ui/fitTitle';
  import { getDetail, peekDetail } from '$lib/discover/details';
  import { genreRu } from '$lib/genres';
  import { motionOn } from '$lib/motion';
  import { toggle, toggleMark } from '$lib/marks';
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
  const backdrop = $derived(selected?.card?.backdrop ?? detail?.backdrop ?? detail?.gallery?.[0] ?? null);
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
  let touchStart: { x: number; y: number } | null = null;
  function hold(e?: PointerEvent) {
    manual = true;
    if (e?.pointerType === 'touch') touchStart = { x: e.clientX, y: e.clientY };
    save();
  }
  function swipe(e: PointerEvent) {
    if (e.pointerType !== 'touch' || !touchStart || items.length < 2) return;
    const dx = e.clientX - touchStart.x, dy = e.clientY - touchStart.y;
    touchStart = null;
    if (Math.abs(dx) > 50 && Math.abs(dx) > Math.abs(dy) * 1.2) { at = (at + (dx < 0 ? 1 : -1) + items.length) % items.length; save(); }
  }
  function save() { try { sessionStorage.setItem('nova-showcase', JSON.stringify({ id: String(selected?.card?.id ?? selected?.ext?.id), manual })); } catch { /* приватный режим */ } }

  // «Кинозал+»: надпись над названием и цвет кадра - по фильму (у своих - цвет постера).
  const accent = '#ffffff';
  // Цвет страницы - по кадру: фон героя и верх рядов ниже плавно переходят в него (см. @property --tone).
  let tone = $state('#15161a');
  $effect(() => {
    const src = selected?.card?.backdropSmall ?? artW(backdrop, 640) ?? backdrop;
    if (!src) return;
    let live = true;
    void toneOfUrl(src).then((c) => { if (live && c) tone = c; });
    return () => { live = false; };
  });
  $effect(() => {
    document.documentElement.style.setProperty('--tone', tone);
    return () => document.documentElement.style.removeProperty('--tone');
  });
  const ext = $derived(selected?.ext);
  const eyebrow = $derived(
    selected?.card?.resume ? 'Вы смотрели'
      : selected?.card ? 'Новое на Nova'
        : ext?.mark === 'want' ? 'В ваших планах'
          : ext?.mark === 'watching' ? 'Вы смотрите'
            : 'Популярно сейчас',
  );
  const wanted = $derived(selected?.card ? selected.card.status === 'want' : ext?.mark === 'want');
  function want(e: Event) {
    e.stopPropagation();
    if (selected?.card) void toggle(selected.card, 'want');
    else if (selected?.ext) void toggleMark(selected.ext, 'want');
  }
  $effect(() => {
    document.documentElement.style.setProperty('--hero-tint', accent);
    return () => document.documentElement.style.removeProperty('--hero-tint');
  });
</script>

{#if card}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <section class="showcase" aria-label="Рекомендуем" style:--acc={accent} bind:this={root} onpointerenter={() => hovering = true} onpointerleave={() => hovering = false} onpointerdown={hold} onpointerup={swipe} onpointercancel={() => touchStart = null} onkeydown={() => hold()}>
    <div class="stage" aria-hidden="true">{#key backdrop}{#if backdrop}<img class="backdrop" in:fade|global={{ duration: 800 }} out:fade|global={{ duration: 800 }} src={backdrop} srcset={heroSet(backdrop, selected?.card?.backdropSmall)} sizes="80vw" alt="" fetchpriority="high" />{/if}{/key}</div>
    <div class="shade"></div>
    <div class="copies">{#key card.id}<div class="copy" in:fly|global={{ y: 16, duration: 520, delay: 220 }} out:fade|global={{ duration: 240 }}>
      <span class="eyebrow">{eyebrow}</span>
      <h1 use:fitTitle={card.title}>{card.title}</h1>
      <p class="meta">
        {#if card.rating}<span class="rating"><Icon name="star" size={14} /> {card.rating.toFixed(1)}</span>{/if}
        {#if card.year}<span>{card.year}</span>{/if}
        {#if card.genres.length}<span>{card.genres.slice(0, 2).map(genreRu).join(', ')}</span>{:else}<span>{card.kind === 'series' ? 'Сериал' : 'Фильм'}</span>{/if}
      </p>
      {#if overview}<p class="description">{overview}</p>{/if}
      <div class="actions">
        {#if selected.card?.resume}<a class="btn primary" href="/watch/{selected.card.resume.fileId}"><Icon name="play" size={18} /> Продолжить</a>{:else}<a class="btn primary" {href}><Icon name="play" size={18} /> Смотреть</a>{/if}
        <button class="btn glass" class:on={wanted} aria-pressed={wanted} onclick={want}><Icon name={wanted ? 'check' : 'plus'} size={18} /> {wanted ? 'В планах' : 'Хочу'}</button>
      </div>
      {#if !detail && selected.ext}<Loading compact label="Подробности загружаются в фоне" />{/if}
    </div>{/key}</div>
    {#if items.length > 1}
      <div class="dots" role="tablist" aria-label="Кадры">
        {#each items as it, i (it.card?.id ?? it.ext?.id)}
          <button role="tab" aria-selected={i === at} class:on={i === at} aria-label={it.card?.title ?? it.ext?.title} onclick={() => { manual = true; at = i; save(); }}></button>
        {/each}
      </div>
    {/if}
  </section>
{/if}

<style>
  /* «Кинозал+» (alpha 0.5): кадр почти на весь экран, слева - надпись цветом фильма, крупное название,
     оценка, описание и две капсулы; справа внизу - точки. Затемнение только слева и снизу. */
  .showcase { position: relative; min-height: clamp(520px, 78svh, 860px); isolation: isolate; display: flex; align-items: flex-end; padding: calc(var(--header) + 36px) var(--gutter) clamp(48px, 9vh, 100px); overflow: hidden; }
  /* Кадр - справа, на 78% ширины: слева он растворяется в цвет фильма (--tone, тот же, что у фона страницы). */
  .stage { position: absolute; inset: 0 0 0 auto; width: 78%; z-index: -2; }
  .backdrop { position: absolute; inset: 0; width: 100%; height: 100%; max-width: none; object-fit: cover; object-position: center 30%; }
  .shade { position: absolute; inset: 0; z-index: -1; pointer-events: none; }
  .stage .backdrop { animation: drift 14s ease-out both; }
  .shade { background: linear-gradient(90deg, var(--tone-dark) 0%, color-mix(in srgb, var(--tone-dark) 90%, transparent) 22%, color-mix(in srgb, var(--tone-dark) 45%, transparent) 44%, transparent 68%), linear-gradient(0deg, var(--tone-dark) 0%, color-mix(in srgb, var(--tone-dark) 50%, transparent) 22%, transparent 50%); }
  .copies { display: grid; max-width: min(46vw, 620px); }
  .copy { grid-area: 1 / 1; align-self: end; }
  @keyframes feature-in { from { opacity: 0; transform: translateY(14px); } }
  @keyframes backdrop-in { from { opacity: 0; } }
  @keyframes drift { from { transform: scale(1.06); } }
  .eyebrow { display: inline-flex; align-items: center; gap: 10px; font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; color: color-mix(in srgb, var(--acc) 65%, #fff); }
  .eyebrow::before { content: ''; width: 22px; height: 2px; background: currentColor; }
  h1 { font-size: clamp(38px, 4.6vw, 72px); line-height: 1.02; letter-spacing: -.035em; font-weight: 800; margin: 14px 0; white-space: nowrap; text-overflow: ellipsis; text-shadow: 0 2px 30px #0006; overflow: hidden; padding-bottom: .08em; }
  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; font-size: 14px; font-weight: 500; color: #c9c4bb; margin: 0; }
  .rating { display: inline-flex; align-items: center; gap: 5px; color: #f5c518; font-weight: 800; }
  .description { max-width: 60ch; font-size: 16px; line-height: 1.6; color: #ddd8d0; display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; margin: 16px 0 0; }
  .actions { display: flex; gap: 12px; flex-wrap: wrap; margin-top: 26px; }
  .actions .btn { height: 52px; padding: 0 26px; border-radius: 999px; font-weight: 700; font-size: 15px; gap: 10px; border: 0; transition: transform .15s ease, background .2s; }
  .actions .btn:active { transform: scale(.96); }
  .actions .btn.primary { background: #fff; color: #0a0a0a; }
  /* Стекло - только на маленькой капсуле. */
  .actions .btn.glass { background: #ffffff1c; color: #fff; box-shadow: inset 0 0 0 1px #ffffff2e; backdrop-filter: blur(14px); }
  .actions .btn.glass:hover { background: #ffffff2a; }
  .actions .btn.glass.on { box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--acc) 70%, #fff); }
  .dots { position: absolute; right: var(--gutter); bottom: clamp(80px, 12vh, 140px); display: flex; gap: 6px; }
  .dots button { width: 8px; height: 8px; padding: 0; border: 0; border-radius: 99px; background: #ffffff55; cursor: pointer; transition: width .3s cubic-bezier(.2,.8,.2,1), background .3s; }
  .dots button.on { width: 28px; background: #fff; }
  :global(.no-motion) .backdrop, :global(.no-motion) .copy { animation: none; }
  /* Телефон: кадр сверху почти целиком, всё по центру под ним. */
  @media (max-width: 640px) {
    .showcase { min-height: 0; touch-action: pan-y; padding: calc(75vw - 40px) var(--gutter) 50px; justify-content: center; text-align: center; }
    .stage { inset: 0 0 auto 0; width: 100%; aspect-ratio: 4 / 3; -webkit-mask-image: linear-gradient(#000 60%, transparent); mask-image: linear-gradient(#000 60%, transparent); }
    .shade { background: linear-gradient(0deg, var(--tone-dark) 30%, #0000 70%), linear-gradient(180deg, #0006, #0000 30%); }
    .copies { max-width: 100%; }
    .eyebrow { justify-content: center; font-size: 11px; }
    h1 { font-size: clamp(32px, 10vw, 44px); margin: 8px 0 10px; }
    .meta { justify-content: center; font-size: 13px; }
    .description { display: none; }
    .actions { justify-content: center; margin-top: 20px; }
    .actions .btn { height: 48px; padding: 0 22px; }
    .dots { left: 0; right: 0; bottom: 18px; justify-content: center; }
  }
</style>
