<script lang="ts">
  import { afterNavigate, goto } from '$app/navigation';
  import { hover, hoverKeep, hoverLeave, toggleList } from '$lib/cards.svelte';
  import { kindLabel, runtime, count } from '$lib/fmt';
  import { placeholder } from './art';
  import Icon from './Icon.svelte';
  import Rate from './Rate.svelte';

  const W = 1.5;
  const c = $derived(hover.card);
  const r = $derived(hover.rect);

  // Карточка шире постера и не вылезает за края окна.
  const box = $derived.by(() => {
    if (!r) return null;
    const w = Math.min(390, Math.max(310, r.width * W * 1.45));
    let left = r.left + r.width / 2 - w / 2;
    left = Math.max(12, Math.min(window.innerWidth - w - 12, left));
    const top = Math.max(76, r.top + r.height / 2 - (w * 0.5625 + 150) / 2);
    const origin = `${((r.left + r.width / 2 - left) / w) * 100}% 50%`;
    return { w, left, top, origin };
  });

  afterNavigate(() => (hover.card = null));

  $effect(() => {
    const close = () => (hover.card = null);
    window.addEventListener('scroll', close, { passive: true });
    window.addEventListener('resize', close);
    return () => {
      window.removeEventListener('scroll', close);
      window.removeEventListener('resize', close);
    };
  });
</script>

{#if c && box}
  {#key c.id}
    <div
      class="hc"
      role="region"
      aria-label={c.title}
      style:left="{box.left}px"
      style:top="{box.top}px"
      style:width="{box.w}px"
      style:transform-origin={box.origin}
      onmouseenter={hoverKeep}
      onmouseleave={hoverLeave}
    >
      <a class="pic" href="/title/{c.id}" style:background={c.backdrop ? undefined : placeholder(c.title)}>
        {#if c.backdrop}<img src={c.backdrop} alt="" />{/if}
        <span class="shade"></span>
        {#if c.logo}<img class="logo" src={c.logo} alt={c.title} />{:else}<b class="name">{c.title}</b>{/if}
      </a>
      <div class="body">
        <div class="actions">
          <button class="round white" aria-label="Смотреть" title="Смотреть" onclick={() => goto(`/title/${c.id}?play=1`)}>
            <Icon name="play" size={18} />
          </button>
          <button class="round" aria-label={c.inList ? 'Убрать из списка' : 'В мой список'} title={c.inList ? 'Убрать из списка' : 'В мой список'} onclick={() => toggleList(c)}>
            <Icon name={c.inList ? 'check' : 'plus'} size={18} />
          </button>
          <Rate card={c} />
          <span class="grow"></span>
          <a class="round" href="/title/{c.id}" aria-label="Подробнее" title="Подробнее"><Icon name="down" size={18} /></a>
        </div>
        <div class="meta">
          {#if c.matchPct}<span class="match">{c.matchPct}% совпадение</span>{/if}
          {#if c.age}<span class="age">{c.age}</span>{/if}
          <span>
            {#if c.kind === 'series'}{count(c.seasons || 1, 'сезон', 'сезона', 'сезонов')}{:else if c.runtime}{runtime(c.runtime)}{:else}{kindLabel(c.kind)}{/if}
          </span>
          {#if c.year}<span>{c.year}</span>{/if}
          {#if c.rating}<span class="imdb">IMDb <b>{c.rating.toFixed(1)}</b></span>{/if}
        </div>
        {#if c.genres.length}
          <div class="genres">{#each c.genres.slice(0, 3) as g, i (g)}{#if i}<i>•</i>{/if}<span>{g}</span>{/each}</div>
        {/if}
      </div>
    </div>
  {/key}
{/if}

<style>
  .hc {
    position: fixed;
    z-index: 60;
    border-radius: 12px;
    overflow: hidden;
    background: #15123a;
    box-shadow:
      0 30px 70px -10px rgba(0, 0, 0, 0.85),
      0 0 0 1px var(--line);
    animation: pop 0.28s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.66);
    }
  }
  .pic {
    position: relative;
    display: block;
    aspect-ratio: 16 / 9;
  }
  .pic img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, #15123a 0%, transparent 45%);
  }
  .logo {
    position: absolute;
    left: 14px;
    bottom: 10px;
    width: auto !important;
    height: auto !important;
    max-width: 55%;
    max-height: 40%;
    object-fit: contain;
  }
  .name {
    position: absolute;
    left: 14px;
    right: 14px;
    bottom: 10px;
    font-size: 19px;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.6);
  }
  .body {
    display: grid;
    gap: 10px;
    padding: 12px 14px 16px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .grow {
    flex: 1;
  }
  .round {
    width: 38px;
    height: 38px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 1.5px solid rgba(255, 255, 255, 0.5);
    background: rgba(0, 0, 0, 0.5);
    color: #fff;
    cursor: pointer;
    transition:
      border-color 0.15s,
      transform 0.15s;
  }
  .round:hover {
    border-color: #fff;
    transform: scale(1.06);
  }
  .round.white {
    background: #fff;
    border-color: #fff;
    color: #0a0a0a;
    padding-left: 2px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    font-size: 13px;
    color: #d6d3ea;
  }
  .match {
    color: var(--ok);
    font-weight: 700;
  }
  .age {
    padding: 0 5px;
    border: 1px solid rgba(255, 255, 255, 0.4);
    border-radius: 3px;
    font-size: 11px;
  }
  .meta .imdb {
    height: 18px;
    font-size: 10px;
  }
  .genres {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    font-size: 13px;
  }
  .genres i {
    font-style: normal;
    color: var(--faint);
  }
</style>
