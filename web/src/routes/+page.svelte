<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { catalog, loadCatalog } from '$lib/catalog.svelte';
  import { counts, featured, filter, ofTab, shelves, STATUSES, statusOf, TABS, tabOf, type Status } from '$lib/library';
  import { session } from '$lib/session.svelte';
  import Hero from '$lib/ui/Hero.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Shelf from '$lib/ui/Shelf.svelte';
  import Wide from '$lib/ui/Wide.svelte';

  // Главная как у Apple TV: герой-карусель и ряды широких карточек. Фильмы и Сериалы — тот же
  // вид плюс фильтр по статусу и вся коллекция сеткой. Каталог — один запрос на все вкладки.
  const tab = $derived(tabOf(page.url.searchParams));
  const status = $derived(statusOf(page.url.searchParams));

  $effect(() => {
    loadCatalog();
  });

  const items = $derived(catalog.items ? ofTab(catalog.items, tab) : null);
  const hero = $derived(items ? featured(items) : []);
  const rows = $derived(items ? shelves(items, catalog.recent, tab) : []);
  const n = $derived(items ? counts(items) : null);
  const shown = $derived(items && tab !== 'home' ? filter(items, status) : []);

  function setStatus(s: Status) {
    const u = new URL(page.url);
    if (s === 'all') u.searchParams.delete('status');
    else u.searchParams.set('status', s);
    goto(u, { replaceState: true, noScroll: true, keepFocus: true });
  }

  const EMPTY: Record<Status, string> = {
    all: 'Здесь пока пусто',
    want: 'Отметьте «Хочу посмотреть» — кнопкой «+» на странице фильма',
    watching: 'Начатого нет — всё досмотрено',
    watched: 'Досмотренное появится здесь',
  };
</script>

<div class="home" class:flat={items !== null && !hero.length}>
  <!-- Ниже 1000 px капсулы в шапке нет — разделы каталога здесь, поверх кадра. -->
  <nav class="kinds" aria-label="Разделы каталога">
    {#each TABS as t (t.id)}
      <a href={t.href} class:on={tab === t.id} aria-current={tab === t.id ? 'page' : undefined}>{t.label}</a>
    {/each}
  </nav>

  {#if catalog.failed}
    <div class="page empty"><Icon name="offline" size={40} /> Не удалось загрузить — проверьте связь</div>
  {:else if items === null}
    <div class="ph-hero skeleton" aria-busy="true"></div>
    <div class="rows">
      {#each Array(2) as _, r (r)}
        <div class="ph-row">
          <span class="ph-title skeleton"></span>
          <div class="ph-cards">{#each Array(6) as _, i (i)}<span class="ph-card skeleton"></span>{/each}</div>
        </div>
      {/each}
    </div>
  {:else if items.length === 0}
    <div class="page empty">
      <Icon name="film" size={40} />
      {#if session.me?.manage}
        Фильмов ещё нет. <a class="btn primary sm" href="/admin?tab=folders">Добавить фильмы</a>
      {:else}
        Здесь пока пусто — хост ещё не добавил фильмы
      {/if}
    </div>
  {:else}
    {#if hero.length}<Hero items={hero} />{/if}

    <div class="rows">
      {#if tab === 'home'}
        {#each rows as row, i (row.key)}<Shelf title={row.title} items={row.items} eager={i < 2} />{/each}
      {:else}
        {#each rows.filter((r) => r.key === 'continue') as row (row.key)}<Shelf title={row.title} items={row.items} eager />{/each}
        <section aria-label="Вся коллекция">
          <div class="head">
            <h2>{tab === 'movies' ? 'Все фильмы' : 'Все сериалы'}</h2>
            <div class="chips" role="tablist" aria-label="Статус">
              {#each STATUSES as s (s.id)}
                <button class="chip" class:on={status === s.id} role="tab" aria-selected={status === s.id} onclick={() => setStatus(s.id)}>
                  {s.label}
                  {#if n && n[s.id]}<small>{n[s.id]}</small>{/if}
                </button>
              {/each}
            </div>
          </div>
          {#if shown.length}
            <div class="grid">
              {#each shown as card, i (card.id)}<Wide {card} eager={i < 8} />{/each}
            </div>
          {:else}
            <div class="empty">{EMPTY[status]}</div>
          {/if}
        </section>
      {/if}
    </div>
  {/if}
</div>

<style>
  .home {
    min-height: 100dvh;
    padding-bottom: calc(48px + var(--tabbar));
  }
  .kinds {
    display: none;
  }
  @media (max-width: 1000px) {
    .kinds {
      position: absolute;
      z-index: 5;
      top: calc(var(--header) + env(safe-area-inset-top) + 4px);
      left: 50%;
      translate: -50% 0;
      display: flex;
      gap: 2px;
      padding: 4px;
      border-radius: 999px;
      background: rgba(48, 44, 42, 0.62);
      backdrop-filter: saturate(180%) blur(24px);
      -webkit-backdrop-filter: saturate(180%) blur(24px);
      box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
    }
    .kinds a {
      padding: 6px 16px;
      border-radius: 999px;
      color: rgba(255, 255, 255, 0.82);
      font-size: 14px;
      font-weight: 600;
      white-space: nowrap;
      transition:
        background 0.2s,
        color 0.2s;
    }
    .kinds a.on {
      background: rgba(235, 235, 240, 0.92);
      color: #111;
    }
    /* Без героя капсула — над содержимым. */
    .flat .kinds {
      position: relative;
      top: auto;
      left: auto;
      translate: none;
      width: max-content;
      margin: calc(var(--header) + env(safe-area-inset-top) + 4px) auto 0;
    }
  }
  /* Ряды «лежат» на нижней части кадра, как у Apple TV. */
  .rows {
    position: relative;
    z-index: 1;
    margin-top: clamp(-150px, -15vh, -96px);
    padding: 0 var(--gutter);
  }
  .flat .rows {
    margin-top: 0;
    padding-top: calc(var(--header) + 24px);
  }
  @media (max-width: 1000px) {
    .flat .rows {
      padding-top: 20px;
    }
  }
  @media (max-width: 640px) {
    .rows {
      margin-top: -64px;
    }
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px 20px;
    margin-bottom: 16px;
  }
  h2 {
    font-size: clamp(19px, 1.6vw, 24px);
    font-weight: 700;
  }
  .chips {
    display: flex;
    gap: 8px;
    min-width: 0;
    overflow-x: auto;
  }
  .ph-hero {
    height: clamp(460px, 78vh, 860px);
    border-radius: 0;
  }
  @media (max-width: 640px) {
    .ph-hero {
      height: clamp(440px, 74vh, 640px);
    }
  }
  .ph-row {
    margin-bottom: 30px;
  }
  .ph-title {
    display: block;
    width: 180px;
    height: 24px;
    border-radius: 6px;
    margin-bottom: 14px;
  }
  .ph-cards {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: calc((100% - 4.3 * 18px) / 5.3);
    gap: 18px;
    overflow: hidden;
  }
  .ph-card {
    aspect-ratio: 16 / 9;
    border-radius: 12px;
  }
  @media (max-width: 640px) {
    .ph-cards {
      grid-auto-columns: calc((100% - 12px) / 1.75);
      gap: 12px;
    }
  }
  .empty :global(.btn) {
    margin-top: 6px;
  }
</style>
