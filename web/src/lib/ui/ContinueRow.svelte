<script lang="ts">
  import type { ResumeItem } from '$lib/types';
  import { clock } from '$lib/fmt';
  import { left, percent } from '$lib/library';
  import { drop, setMark } from '$lib/marks';
  import { artW, placeholder } from './art';
  import { toast } from './toast.svelte';
  import Icon from './Icon.svelte';

  // «Продолжить просмотр» в «Кинозале»: широкие кадры 16:9 с полосой просмотренного; название и
  // где остановились - под кадром.
  let { items }: { items: ResumeItem[] } = $props();
  const shown = $derived(items.filter((r) => r.card?.status !== 'dropped' && r.ext?.mark !== 'dropped'));
  async function remove(item: ResumeItem) {
    const title = (item.card ?? item.ext)?.title ?? '';
    if (item.card) await drop(item.card, title);
    else if (item.ext) {
      const card = item.ext;
      await setMark(card, 'dropped');
      toast(`«${title}» - больше не смотрю`, 6000, { label: 'Вернуть', run: () => setMark(card, 'watching') });
    }
  }
</script>

{#if shown.length}
  <section class="continue" aria-label="Продолжить просмотр">
    <h2>Продолжить просмотр</h2>
    <div class="strip">
      {#each shown as item (item.key)}
        {@const card = item.card ?? item.ext!}
        {@const art = item.art ?? card.poster}
        <article style:--acc={item.card?.accent ?? '#c8894a'}>
          <a href={item.href} aria-label="{card.title} - {item.label} - {left(item.position, item.duration)}">
            <span class="art" style:background={art ? undefined : placeholder(card.title)}>
              {#if art}<img src={artW(art, 640)} alt="" loading="lazy" decoding="async" width="640" height="360" />{:else}<span class="ph">{card.title}</span>{/if}
              <span class="play" aria-hidden="true"><Icon name="play" size={18} /></span>
              <span class="track" role="meter" aria-label="Просмотрено" aria-valuenow={Math.round(percent(item.position, item.duration))} aria-valuemin={0} aria-valuemax={100}><i style:width="{percent(item.position, item.duration)}%"></i></span>
            </span>
            <b>{card.title}</b>
            <span class="meta">{item.label}{item.position === 0 ? ' · дальше' : ''}</span>
            <small>{#if item.position > 0}{clock(item.position)} · {/if}{left(item.position, item.duration)}</small>
          </a>
          <button class="remove" aria-label="Больше не смотрю: {card.title}" title="Убрать из продолжения" onclick={() => remove(item)}><Icon name="close" size={14} /></button>
        </article>
      {/each}
    </div>
  </section>
{/if}

<style>
  .continue {
    margin-bottom: 30px;
  }
  h2 {
    margin: 0 0 12px;
    font-size: clamp(17px, 1.5vw, 22px);
    font-weight: 700;
  }
  .strip {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, calc((100% - 3 * 14px) / 4.2));
    gap: 12px;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    padding-bottom: 6px;
  }
  article {
    position: relative;
    min-width: 0;
    scroll-snap-align: start;
  }
  a {
    display: grid;
    gap: 2px;
    color: inherit;
  }
  .art {
    position: relative;
    display: block;
    aspect-ratio: 16 / 9;
    overflow: hidden;
    margin-bottom: 8px;
    border-radius: 16px;
    background: #111;
    box-shadow: 0 18px 40px -22px #000;
  }
  /* «Кинозал+»: низ кадра темнее - полоса просмотренного читается на любом кадре. */
  .art::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, #000a, transparent 45%);
    pointer-events: none;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.3s;
  }
  a:hover img {
    transform: scale(1.04);
  }
  .ph {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 10px;
    font-weight: 700;
    text-align: center;
  }
  .play {
    position: absolute;
    z-index: 1;
    left: 12px;
    bottom: 20px;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.72);
  }
  .track {
    position: absolute;
    z-index: 1;
    left: 12px;
    right: 12px;
    bottom: 10px;
    height: 4px;
    overflow: hidden;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.25);
  }
  /* Полоса - цветом постера фильма. */
  .track i {
    display: block;
    height: 100%;
    border-radius: 4px;
    background: color-mix(in srgb, var(--acc) 75%, #fff);
  }
  .meta {
    overflow: hidden;
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  b {
    overflow: hidden;
    font-size: 15px;
    font-weight: 700;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  small {
    font-size: 11px;
    color: var(--faint);
  }
  .remove {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.7);
    color: #fff;
    cursor: pointer;
  }
  @media (max-width: 1200px) {
    .strip {
      grid-auto-columns: minmax(0, calc((100% - 2 * 12px) / 3.2));
    }
  }
  @media (max-width: 900px) {
    .strip {
      grid-auto-columns: minmax(0, calc((100% - 10px) / 2.3));
      gap: 10px;
    }
  }
  @media (max-width: 600px) {
    .strip {
      grid-auto-columns: 72%;
      gap: 8px;
    }
  }
</style>
