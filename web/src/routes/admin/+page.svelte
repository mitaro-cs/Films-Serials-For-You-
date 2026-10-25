<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { api, errorText } from '$lib/api';
  import Access from '$lib/admin/Access.svelte';
  import Catalog from '$lib/admin/Catalog.svelte';
  import Folders from '$lib/admin/Folders.svelte';
  import Overview from '$lib/admin/Overview.svelte';
  import People from '$lib/admin/People.svelte';
  import Settings from '$lib/admin/Settings.svelte';
  import Version from '$lib/admin/Version.svelte';
  import Wishes from '$lib/admin/Wishes.svelte';
  import { badges } from '$lib/social/badges.svelte';
  import type { Status } from '$lib/admin/types';
  import { session } from '$lib/session.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import type { IconName } from '$lib/ui/icons';

  const TABS: [string, string, IconName][] = [
    ['overview', 'Обзор', 'pulse'],
    ['folders', 'Папки', 'folder'],
    ['catalog', 'Каталог', 'film'],
    ['people', 'Люди', 'users'],
    ['wishes', 'Просьбы', 'flag'],
    ['access', 'Доступ', 'globe'],
    ['settings', 'Настройки', 'sliders'],
    ['version', 'Версия', 'download'],
  ];

  // Что в каждом разделе и что там можно сделать - хост видит это сверху раздела.
  const INFO: Record<string, { hint: string; title: string; text: string; can: string[] }> = {
    overview: { hint: 'Всё ли работает', title: 'Обзор', text: 'Состояние Nova одним взглядом: что работает, что требует внимания и кто сейчас смотрит.', can: ['Проверить, всё ли в порядке', 'Скопировать адрес для друзей', 'Увидеть, кто и что смотрит'] },
    folders: { hint: 'Где лежат фильмы', title: 'Папки', text: 'Папки на этом компьютере или внешнем диске, из которых Nova берёт фильмы и сериалы. Новые файлы подхватываются сами.', can: ['Добавить или убрать папку', 'Перетащить файлы в своё место для фильмов', 'Посмотреть, как называть папки и файлы'] },
    catalog: { hint: 'Описания и обложки', title: 'Каталог', text: 'Все фильмы и сериалы на сайте. Если описание нашлось не то - выберите верное; можно сменить обложку и фон или скрыть фильм.', can: ['Найти правильное описание', 'Изменить название, год, жанры', 'Скрыть от зрителей или убрать фильм без файлов'] },
    people: { hint: 'Зрители и права', title: 'Люди', text: 'Кто может заходить в Nova. Приглашайте ссылкой, сбрасывайте пароль, давайте право скачивать.', can: ['Пригласить ссылкой или добавить вручную', 'Сбросить пароль без почты', 'Закрыть доступ или удалить'] },
    wishes: { hint: 'Что просят добавить', title: 'Просьбы', text: 'Фильмы, которые зрители попросили добавить. Положите фильм в папку - просьба уйдёт сама.', can: ['Посмотреть, что просят', 'Отклонить просьбу'] },
    access: { hint: 'Вход из интернета', title: 'Доступ', text: 'Как друзья попадают в Nova не из вашей домашней сети. Рекомендуемый способ - CloudPub: постоянный адрес без настройки роутера.', can: ['Включить доступ из интернета', 'Сменить способ доступа', 'Узнать адрес в домашней сети'] },
    settings: { hint: 'Сервер и источники', title: 'Настройки', text: 'Как Nova работает: регистрация, откуда брать описания, ffmpeg, свои сайты с видео, почта, ночной режим.', can: ['Открыть или закрыть регистрацию', 'Выбрать источник описаний', 'Указать путь к ffmpeg'] },
    version: { hint: 'Обновления', title: 'Версия', text: 'Какая версия Nova установлена и как обновиться. Фильмы, люди и настройки при обновлении сохраняются.', can: ['Узнать версию', 'Обновить Nova'] },
  };

  const tab = $derived(page.url.searchParams.get('tab') ?? 'overview');
  const info = $derived(INFO[tab] ?? INFO.overview);
  let status = $state<Status | null>(null);
  let statusError = $state('');

  function reload() {
    api<Status>('/api/admin/status').then((s) => { status = s; statusError = ''; }, (e) => { statusError = errorText(e); });
  }
  $effect(reload);

  $effect(() => {
    if (session.me && !session.me.manage) goto('/', { replace: true });
  });
</script>

<div class="page admin">
  <aside>
    <h1>Управление</h1>
    <nav>
      {#each TABS as [id, label, icon] (id)}
        <a href="?tab={id}" class:on={tab === id} data-sveltekit-replacestate data-sveltekit-noscroll>
          <span class="ic"><Icon name={icon} size={18} /></span><span class="lbl">{label}<small>{INFO[id]?.hint}</small></span>
          {#if id === 'version' && status?.update?.state === 'available'}<i class="new" aria-label="есть обновление"></i>{/if}
          {#if id === 'wishes' && badges.wishes}<i class="new" aria-label="есть просьбы"></i>{/if}
        </a>
      {/each}
    </nav>
    {#if status}<a class="faint ver" href="?tab=version">Версия {status.version}</a>{/if}
  </aside>
  <section>
    <header class="tab-head">
      <h2>{info.title}</h2>
      <p>{info.text}</p>
      <ul aria-label="Что здесь можно сделать">{#each info.can as c (c)}<li><Icon name="check" size={14} /> {c}</li>{/each}</ul>
    </header>
    {#if statusError}<p class="error" role="alert">Не удалось обновить показатели: {statusError} <button class="btn sm" onclick={reload}>Повторить</button></p>{/if}
    {#if status}
      {#if tab === 'overview'}<Overview {status} {reload} />
      {:else if tab === 'folders'}<Folders {status} window={!!session.me?.window} {reload} />
      {:else if tab === 'catalog'}<Catalog />
      {:else if tab === 'people'}<People />
      {:else if tab === 'wishes'}<Wishes />
      {:else if tab === 'access'}<Access />
      {:else if tab === 'settings'}<Settings />
      {:else if tab === 'version'}<Version {status} {reload} />
      {/if}
    {/if}
  </section>
</div>

<style>
  .admin {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 32px;
    max-width: 1200px;
    margin: 0 auto;
  }
  aside {
    position: sticky;
    top: calc(var(--header) + 24px);
    align-self: start;
  }
  h1 {
    font-size: 26px;
    margin-bottom: 18px;
  }
  nav {
    display: grid;
    gap: 4px;
  }
  nav a {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 12px;
    color: var(--muted);
    font-weight: 600;
    font-size: 14px;
  }
  nav a:hover {
    color: var(--text);
    background: var(--surface);
  }
  .ic {
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    border-radius: 10px;
    background: var(--surface);
  }
  nav a.on {
    color: var(--text);
  }
  nav a.on .ic {
    background: var(--accent-grad);
    color: var(--on-accent);
  }
  .ver {
    display: block;
    font-size: 12px;
    margin-top: 18px;
  }
  .new {
    width: 8px;
    height: 8px;
    margin-left: auto;
    border-radius: 50%;
    background: #ff5c7a;
  }
  @media (max-width: 860px) {
    .admin {
      /* minmax(0, …): лента разделов прокручивается, а не распирает страницу. */
      grid-template-columns: minmax(0, 1fr);
      gap: 18px;
    }
    aside {
      position: static;
    }
    nav {
      display: flex;
      overflow-x: auto;
      margin: 0 calc(var(--gutter) * -1);
      padding: 0 var(--gutter);
    }
    nav a {
      white-space: nowrap;
    }
    .ver {
      display: none;
    }
  }

  .admin { max-width: 1800px; grid-template-columns: 240px minmax(0, 1fr); gap: 28px; }
  aside { padding: 22px 16px; border: 1px solid var(--line); border-radius: 22px; background: var(--panel); }
  h1 { padding-left: 10px; font-size: 23px; }
  nav a { padding: 10px; border: 1px solid transparent; }
  nav a.on { background: #c4ceef12; border-color: #c4ceef26; }
  section { min-width: 0; }
  @media(max-width: 860px) { .admin { grid-template-columns: minmax(0, 1fr); } aside { padding: 16px; } nav { margin: 0; padding: 0; } }

  /* Подпись под пунктом меню и шапка раздела с пояснением. */
  .lbl {
    display: grid;
    min-width: 0;
  }
  .lbl small {
    font-size: 11.5px;
    font-weight: 500;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tab-head {
    margin-bottom: 18px;
    padding: 4px 2px 16px;
    border-bottom: 1px solid var(--line);
  }
  .tab-head h2 {
    margin: 0 0 6px;
    font-size: 26px;
  }
  .tab-head p {
    max-width: 72ch;
    margin: 0 0 10px;
    color: var(--muted);
  }
  .tab-head ul {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 13px;
    color: var(--faint);
  }
  .tab-head li {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  @media (max-width: 860px) {
    .lbl small {
      display: none;
    }
  }
</style>
