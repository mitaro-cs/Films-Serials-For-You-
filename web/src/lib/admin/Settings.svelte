<script lang="ts">
  import { api, errorText, post, put } from '$lib/api';
  import { loadMe } from '$lib/session.svelte';
  import Icon from '$lib/ui/Icon.svelte';

  interface S {
    siteName: string;
    registration: 'invite' | 'open' | 'closed';
    downloads: boolean;
    provider: 'imdb' | 'tmdb' | 'kinopoisk' | 'off';
    tmdbKeySet: boolean;
    kpKeySet: boolean;
    vkTokenSet: boolean;
    tmdbApi: string;
    tmdbImages: string;
    ffmpegPath: string;
    ffmpeg: string | null;
    ffprobe: string | null;
  }

  let s = $state<S | null>(null);
  let tmdbKey = $state('');
  let kpKey = $state('');
  let vkToken = $state('');
  let msg = $state('');
  let error = $state('');

  function load() {
    api<S>('/api/admin/settings').then((x) => (s = x), (e) => (error = errorText(e)));
  }
  $effect(load);

  async function save(body: Record<string, unknown>, note = 'Сохранено') {
    error = msg = '';
    try {
      await put('/api/admin/settings', body);
      msg = note;
      load();
      loadMe();
      setTimeout(() => (msg = ''), 2000);
    } catch (x) {
      error = errorText(x);
    }
  }

  async function refreshAll() {
    const r = await post<{ count: number }>('/api/admin/meta/refresh').catch(() => null);
    msg = r ? `Обновляем описания: ${r.count}` : '';
  }
</script>

{#if s}
  {#if msg}<p class="ok">{msg}</p>{/if}
  {#if error}<p class="error">{error}</p>{/if}

  <section class="card pane">
    <h3>Сайт</h3>
    <form class="line" onsubmit={(e) => (e.preventDefault(), save({ siteName: s!.siteName }))}>
      <label class="field grow"><span>Название</span><input class="input" bind:value={s.siteName} maxlength="60" /></label>
      <button class="btn">Сохранить</button>
    </form>
    <label class="field">
      <span>Регистрация</span>
      <select class="input" value={s.registration} onchange={(e) => save({ registration: (e.target as HTMLSelectElement).value })}>
        <option value="invite">Только по приглашению</option>
        <option value="open">Открытая — любой, у кого есть адрес</option>
        <option value="closed">Закрыта — только те, кого добавили вы</option>
      </select>
    </label>
    <label class="check">
      <input type="checkbox" checked={s.downloads} onchange={(e) => save({ downloads: (e.target as HTMLInputElement).checked })} />
      Разрешить скачивать фильмы
    </label>
  </section>

  <section class="card pane">
    <h3>Описания, постеры и актёры</h3>
    <div class="providers">
      {#each [['imdb', 'IMDb', 'Без ключа. Рейтинг IMDb, актёры, кадры; сюжет — на английском'], ['tmdb', 'TMDB', 'Нужен бесплатный ключ. Описания на русском, фоны, логотипы'], ['kinopoisk', 'Кинопоиск', 'Ключ api.kinopoisk.dev. Всё на русском, 200 запросов в день'], ['off', 'Выключено', 'Только названия из имён файлов']] as [id, name, note] (id)}
        <button class="prov" class:on={s.provider === id} onclick={() => save({ provider: id })}>
          <b>{name}</b><span class="muted">{note}</span>
        </button>
      {/each}
    </div>
    {#if s.provider === 'tmdb'}
      <form class="line" onsubmit={(e) => (e.preventDefault(), save({ tmdbKey }, 'Ключ сохранён'), (tmdbKey = ''))}>
        <label class="field grow">
          <span>Ключ TMDB (API key или Read Access Token) {s.tmdbKeySet ? '· задан' : ''}</span>
          <input class="input" bind:value={tmdbKey} placeholder={s.tmdbKeySet ? '••••••••' : 'themoviedb.org → Настройки → API'} autocomplete="off" />
        </label>
        <button class="btn">Сохранить</button>
      </form>
      <details>
        <summary class="muted">Зеркала TMDB (если сайт не открывается)</summary>
        <form class="line" onsubmit={(e) => (e.preventDefault(), save({ tmdbApi: s!.tmdbApi, tmdbImages: s!.tmdbImages }))}>
          <input class="input grow" bind:value={s.tmdbApi} placeholder="https://api.themoviedb.org/3" />
          <input class="input grow" bind:value={s.tmdbImages} placeholder="https://image.tmdb.org/t/p" />
          <button class="btn">Сохранить</button>
        </form>
      </details>
    {:else if s.provider === 'kinopoisk'}
      <form class="line" onsubmit={(e) => (e.preventDefault(), save({ kpKey }, 'Ключ сохранён'), (kpKey = ''))}>
        <label class="field grow">
          <span>Ключ api.kinopoisk.dev {s.kpKeySet ? '· задан' : ''}</span>
          <input class="input" bind:value={kpKey} placeholder={s.kpKeySet ? '••••••••' : 'Ключ из бота @kinopoiskdev_bot'} autocomplete="off" />
        </label>
        <button class="btn">Сохранить</button>
      </form>
    {/if}
    <button class="btn sm" onclick={refreshAll}><Icon name="refresh" size={14} /> Обновить все описания</button>
  </section>

  <section class="card pane">
    <h3>Видео по ссылке</h3>
    <p class="muted">
      Поиск на Rutube работает без ключа. Для поиска в VK Видео нужен сервисный ключ приложения VK
      (vk.com/apps?act=manage → своё приложение → «Сервисный ключ доступа»).
    </p>
    <form class="line" onsubmit={(e) => (e.preventDefault(), save({ vkToken }, 'Ключ сохранён'), (vkToken = ''))}>
      <label class="field grow">
        <span>Ключ VK {s.vkTokenSet ? '· задан' : ''}</span>
        <input class="input" bind:value={vkToken} placeholder={s.vkTokenSet ? '••••••••' : 'Сервисный ключ доступа'} autocomplete="off" />
      </label>
      <button class="btn">Сохранить</button>
    </form>
  </section>

  <section class="card pane">
    <h3>ffmpeg</h3>
    <p class="muted">
      {#if s.ffmpeg}Найден: <code>{s.ffmpeg}</code>{:else}Не найден. Без него браузер играет только MP4 и WebM.{/if}
    </p>
    <form class="line" onsubmit={(e) => (e.preventDefault(), save({ ffmpegPath: s!.ffmpegPath }))}>
      <input class="input grow" bind:value={s.ffmpegPath} placeholder="Путь к ffmpeg (если не нашёлся сам)" />
      <button class="btn">Сохранить</button>
    </form>
  </section>
{/if}

<style>
  .pane {
    display: grid;
    gap: 14px;
    padding: 18px 20px;
    margin-bottom: 14px;
  }
  h3 {
    font-size: 16px;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 10px;
  }
  .grow {
    flex: 1 1 220px;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 14px;
  }
  .providers {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 10px;
  }
  .prov {
    display: grid;
    gap: 4px;
    padding: 14px;
    text-align: left;
    border-radius: var(--radius-sm);
    border: 1px solid var(--line);
    background: rgba(0, 0, 0, 0.2);
    cursor: pointer;
  }
  .prov span {
    font-size: 12px;
  }
  .prov.on {
    border-color: var(--accent-2);
    background: rgba(255, 255, 255, 0.14);
  }
  .pane > .btn {
    justify-self: start;
  }
  p {
    margin: 0;
  }
  code {
    padding: 1px 6px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.35);
    font-size: 13px;
  }
  .ok {
    color: var(--ok);
  }
  details summary {
    cursor: pointer;
    font-size: 13px;
    margin-bottom: 10px;
  }
</style>
