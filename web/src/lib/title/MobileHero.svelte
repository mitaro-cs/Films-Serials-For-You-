<script lang="ts">
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import { fadeIn } from '$lib/ui/fade';
  import { fitTitle } from '$lib/ui/fitTitle';
  import { artW, placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import TrailerBanner from './TrailerBanner.svelte';

  // Верх страницы фильма и сериала на телефоне (по макету Fallout, белый вместо синего): кадр на всю
  // ширину с кнопками «назад» и «…», по центру название, плашки (оценка, «Новый сезон»/«Смотрю»),
  // строка сведений и описание, большая белая кнопка («Сезон 1, серия 2» / «Смотреть»), четыре
  // действия с подписями: трейлер, оценить, сохранить, поделиться. У сериала ниже - сезоны и серии.
  let {
    title,
    art,
    tall = false,
    fallback = null,
    imdbId = null,
    meta = [],
    rating = null,
    badge = null,
    overview = '',
    genres = [],
    play = null,
    wanted = false,
    wantDisabled = false,
    onwant,
    onmore,
    onclose,
  }: {
    title: string;
    /** Кадр фильма (16:9) или, если кадра нет, постер. */
    art: string | null;
    /** В `art` - постер (вертикальный): показываем верхнюю часть. */
    tall?: boolean;
    /** Пока кадр грузится (первый раз сервер качает его с IMDb) - постер, он у Nova уже есть. */
    fallback?: string | null;
    /** Для кнопки «Трейлер». */
    imdbId?: string | null;
    meta?: string[];
    rating?: number | null;
    /** Белая плашка рядом с оценкой: «Новый сезон», «Смотрю», «Просмотрено». */
    badge?: string | null;
    overview?: string;
    genres?: string[];
    play?: { label: string; href?: string; onclick?: () => void; note?: string } | null;
    wanted?: boolean;
    wantDisabled?: boolean;
    onwant?: () => void;
    /** «…» и «Оценить»: окно со всеми действиями и звёздами. */
    onmore?: () => void;
    onclose?: () => void;
  } = $props();

  // Кадр IMDb приходит шириной до 3840 px - телефону хватает 1280 (грузится в разы быстрее).
  const src = $derived(art?.replace(/_UX\d{4}_/, '_UX1280_') ?? null);

  // Трейлер в кадре без звука (как в макете), звук - круглой кнопкой справа сверху.
  let banner = $state<ReturnType<typeof TrailerBanner>>();
  let hasTrailer = $state(false);
  let muted = $state(true);

  let trailerBusy = $state(false);
  // Русского трейлера нет - официальный HD с IMDb (точно этот фильм) уже идёт в кадре: включаем звук.
  function hdTrailer() {
    if (hasTrailer && muted) banner?.toggleSound();
    else if (!hasTrailer) toast('Трейлер не нашёлся');
  }
  async function trailer() {
    if (!imdbId || trailerBusy) return;
    trailerBusy = true;
    try {
      const r = await api<{ linkId: number | null }>(`/api/discover/title/${imdbId}/trailer`);
      if (r.linkId) goto(`/watch/link/${r.linkId}`);
      else hdTrailer();
    } catch {
      hdTrailer();
    } finally {
      trailerBusy = false;
    }
  }

  async function share() {
    const url = location.href;
    try {
      if (navigator.share) return void (await navigator.share({ title, url }));
      await navigator.clipboard.writeText(url);
      toast('Ссылка скопирована');
    } catch {
      /* отменили - ничего не делаем */
    }
  }
</script>

<section class="mh" aria-label={title}>
  <div class="art" class:tall style:background={art ? undefined : placeholder(title)} style:background-image={art && fallback ? `url("${fallback}")` : undefined}>
    {#if imdbId}
      <TrailerBanner bind:this={banner} bind:available={hasTrailer} bind:muted {imdbId} poster={src} bare />
    {:else if src}<img use:fadeIn src={artW(src, 1280)} alt="" decoding="async" fetchpriority="high" />{/if}
    <div class="shade"></div>
    {#if onclose}<button class="rnd back" aria-label="Назад" onclick={onclose}><Icon name="back" size={20} /></button>{/if}
    {#if hasTrailer}
      <button class="rnd more" aria-label={muted ? 'Включить звук трейлера' : 'Выключить звук трейлера'} onclick={() => banner?.toggleSound()}><Icon name={muted ? 'mute' : 'volume'} size={20} /></button>
    {:else if onmore}<button class="rnd more" aria-label="Ещё действия" onclick={onmore}><Icon name="dots" size={20} /></button>{/if}
    <h1 use:fitTitle={title} {title}>{title}</h1>
  </div>
  <div class="body">
    {#if rating || badge}
      <p class="chips">
        {#if rating}<span class="score" title="Оценка IMDb">{rating.toFixed(1)}</span>{/if}
        {#if badge}<span class="badge">{badge}</span>{/if}
      </p>
    {/if}
    {#if meta.length || genres.length}<p class="meta">{#each [...meta, ...genres.slice(0, 1)].filter(Boolean) as m, k (k)}<span>{m}</span>{/each}</p>{/if}
    {#if overview}<p class="ov">{overview}</p>{/if}
    {#if play}
      {#if play.href}
        <a class="play" href={play.href}><Icon name="play" size={18} /> {play.label}</a>
      {:else}
        <button class="play" onclick={play.onclick}><Icon name="play" size={18} /> {play.label}</button>
      {/if}
      {#if play.note}<p class="note">{play.note}</p>{/if}
    {/if}
    <div class="acts">
      {#if imdbId}<button disabled={trailerBusy} onclick={trailer}><Icon name="film" size={22} /><span>Трейлер</span></button>{/if}
      {#if onmore}<button onclick={onmore}><Icon name="star" size={22} /><span>Оценить</span></button>{/if}
      {#if onwant}
        <button class:on={wanted} aria-pressed={wanted} disabled={wantDisabled} aria-label={wanted ? 'Убрать из «Хочу посмотреть»' : 'Хочу посмотреть'} onclick={onwant}>
          <Icon name={wanted ? 'check' : 'bookmark'} size={22} /><span>{wanted ? 'Сохранено' : 'Сохранить'}</span>
        </button>
      {/if}
      <button onclick={share}><Icon name="share" size={22} /><span>Поделиться</span></button>
    </div>
  </div>
</section>

<style>
  .mh {
    display: none;
  }
  @media (max-width: 640px) {
    .mh {
      display: grid;
    }
    .art {
      position: relative;
      overflow: hidden;
      height: clamp(380px, 62svh, 560px);
      /* Стык кадра и фона без светлой полоски: кадр чуть заходит под блок ниже. */
      margin-bottom: -2px;
      background-color: #111;
      background-size: cover;
      background-position: center 20%;
      animation: art-in 0.5s ease both;
    }
    img {
      position: absolute;
      inset: 0;
      width: 100%;
      height: 100%;
      object-fit: cover;
      object-position: center 25%;
    }
    .tall img {
      object-position: center top;
    }
    .shade {
      position: absolute;
      z-index: 1;
      inset: 0;
      pointer-events: none;
      background:
        linear-gradient(180deg, rgba(0, 0, 0, 0.45), transparent 20%),
        linear-gradient(0deg, #000 0, #000 6%, rgba(0, 0, 0, 0.7) 30%, transparent 58%);
    }
    .rnd {
      position: absolute;
      z-index: 2;
      top: calc(env(safe-area-inset-top) + 12px);
      display: grid;
      place-items: center;
      width: 40px;
      height: 40px;
      border: 1px solid rgba(255, 255, 255, 0.18);
      border-radius: 50%;
      background: rgba(30, 30, 32, 0.55);
      -webkit-backdrop-filter: blur(18px);
      backdrop-filter: blur(18px);
      color: #fff;
      cursor: pointer;
      transition: transform 0.15s ease;
    }
    .rnd:active {
      transform: scale(0.92);
    }
    .back {
      left: 14px;
    }
    .more {
      right: 14px;
    }
    h1 {
      position: absolute;
      z-index: 2;
      left: 20px;
      right: 20px;
      bottom: 10px;
      margin: 0;
      font-size: clamp(32px, 11vw, 48px);
      font-weight: 900;
      line-height: 1;
      letter-spacing: -0.04em;
      text-align: center;
      text-shadow: 0 2px 22px rgba(0, 0, 0, 0.7);
      /* Одной строкой, как логотип в макете: длинное название уменьшается (fitTitle). */
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      animation: rise-in 0.45s cubic-bezier(0.2, 0.8, 0.2, 1) 0.06s both;
    }
    .body {
      display: grid;
      justify-items: center;
      gap: 14px;
      padding: 14px 20px 6px;
      text-align: center;
      animation: rise-in 0.45s cubic-bezier(0.2, 0.8, 0.2, 1) 0.12s both;
    }
    .chips {
      display: flex;
      flex-wrap: wrap;
      justify-content: center;
      gap: 8px;
      margin: 0;
    }
    .score,
    .badge {
      padding: 3px 9px;
      border-radius: 7px;
      font-size: 14px;
      font-weight: 700;
      line-height: 1.3;
    }
    .score {
      background: #30d158;
      color: #04210c;
      font-variant-numeric: tabular-nums;
    }
    .badge {
      background: #fff;
      color: #0a0a0a;
    }
    .meta {
      display: flex;
      flex-wrap: wrap;
      justify-content: center;
      gap: 4px 16px;
      margin: 0;
      font-size: 14px;
      color: rgba(235, 235, 245, 0.62);
    }
    .ov {
      display: -webkit-box;
      margin: 0;
      overflow: hidden;
      font-size: 15px;
      line-height: 1.45;
      color: rgba(255, 255, 255, 0.9);
      -webkit-line-clamp: 4;
      line-clamp: 4;
      -webkit-box-orient: vertical;
    }
    .play {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 10px;
      width: 100%;
      min-height: 52px;
      margin-top: 4px;
      padding: 0 20px;
      border: 0;
      border-radius: 14px;
      background: #fff;
      color: #0a0a0a;
      font: inherit;
      font-size: 17px;
      font-weight: 700;
      cursor: pointer;
      transition: transform 0.15s ease, opacity 0.2s ease;
    }
    .play:active {
      transform: scale(0.98);
      opacity: 0.9;
    }
    .note {
      margin: -6px 0 0;
      font-size: 13px;
      color: rgba(235, 235, 245, 0.62);
    }
    .acts {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(0, 1fr));
      width: 100%;
      max-width: 360px;
      margin-top: 2px;
    }
    .acts button {
      display: grid;
      justify-items: center;
      gap: 4px;
      padding: 8px 2px;
      border: 0;
      background: none;
      color: rgba(255, 255, 255, 0.86);
      font: inherit;
      cursor: pointer;
      transition: transform 0.15s ease, color 0.2s ease;
    }
    .acts button:active {
      transform: scale(0.9);
    }
    .acts button.on {
      color: #fff;
    }
    .acts button:disabled {
      opacity: 0.45;
    }
    .acts span {
      font-size: 12px;
      color: rgba(235, 235, 245, 0.62);
    }
    @keyframes art-in {
      from {
        opacity: 0;
      }
    }
    @keyframes rise-in {
      from {
        opacity: 0;
        transform: translateY(10px);
      }
    }
  }
</style>
