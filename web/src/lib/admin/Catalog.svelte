<script lang="ts">
  import { api, errorText, patch, post } from '$lib/api';
  import { kindLabel } from '$lib/fmt';
  import { placeholder } from '$lib/ui/art';
  import Icon from '$lib/ui/Icon.svelte';
  import Modal from '$lib/ui/Modal.svelte';
  import { ask } from '$lib/ui/dialogs.svelte';

  interface Row {
    id: number;
    kind: 'movie' | 'series';
    title: string;
    year: number | null;
    poster: string | null;
    status: 'pending' | 'matched' | 'none' | 'manual';
    hidden: boolean;
    locked: boolean;
    provider: string | null;
    files: number;
    path: string | null;
  }
  interface Cand {
    id: string;
    title: string;
    original: string | null;
    year: number | null;
    series: boolean;
    overview: string | null;
    poster: string | null;
  }

  let rows = $state<Row[]>([]);
  let q = $state('');
  let filter = $state('');
  let error = $state('');

  let editing = $state<Row | null>(null);
  let editOpen = $state(false);
  let form = $state({ title: '', year: '', kind: 'movie', overview: '', hidden: false, age: '' });

  let matching = $state<Row | null>(null);
  let matchOpen = $state(false);
  let mq = $state('');
  let my = $state('');
  let cands = $state<Cand[] | null>(null);
  let busy = $state(false);

  const STATUS: Record<string, string> = { pending: 'ищем описание', matched: 'найдено', none: 'не найдено', manual: 'выбрано вручную' };

  function load() {
    const p = new URLSearchParams();
    if (q.trim()) p.set('q', q.trim());
    if (filter) p.set('status', filter);
    api<Row[]>(`/api/admin/titles?${p}`).then((r) => (rows = r), (e) => (error = errorText(e)));
  }

  $effect(() => {
    void q;
    void filter;
    const t = setTimeout(load, 200);
    return () => clearTimeout(t);
  });

  async function openEdit(r: Row) {
    editing = r;
    const d = await api<{ card: { overview: string | null; age: string | null } }>(`/api/titles/${r.id}`).catch(() => null);
    form = {
      title: r.title,
      year: r.year ? String(r.year) : '',
      kind: r.kind,
      overview: d?.card.overview ?? '',
      hidden: r.hidden,
      age: d?.card.age ?? '',
    };
    editOpen = true;
  }

  async function saveEdit(e: SubmitEvent) {
    e.preventDefault();
    if (!editing) return;
    error = '';
    try {
      await patch(`/api/admin/titles/${editing.id}`, {
        title: form.title,
        year: form.year ? Number(form.year) : null,
        kind: form.kind,
        overview: form.overview,
        hidden: form.hidden,
        age: form.age,
      });
      editOpen = false;
      load();
    } catch (x) {
      error = errorText(x);
    }
  }

  async function upload(which: 'poster' | 'backdrop', ev: Event) {
    const f = (ev.target as HTMLInputElement).files?.[0];
    if (!f || !editing) return;
    try {
      await api(`/api/admin/titles/${editing.id}/image/${which}`, { method: 'PUT', body: f });
      load();
    } catch (x) {
      error = errorText(x);
    }
  }

  function openMatch(r: Row) {
    matching = r;
    mq = r.title;
    my = r.year ? String(r.year) : '';
    cands = null;
    matchOpen = true;
    search();
  }

  async function search(e?: SubmitEvent) {
    e?.preventDefault();
    if (!matching) return;
    busy = true;
    error = '';
    try {
      const p = new URLSearchParams({ q: mq, series: String(matching.kind === 'series') });
      if (my) p.set('year', my);
      cands = (await api<{ results: Cand[] }>(`/api/admin/titles/${matching.id}/search?${p}`)).results;
    } catch (x) {
      error = errorText(x);
      cands = [];
    } finally {
      busy = false;
    }
  }

  async function choose(c: Cand) {
    if (!matching) return;
    busy = true;
    try {
      await post(`/api/admin/titles/${matching.id}/match`, { id: c.id });
      matchOpen = false;
      load();
    } catch (x) {
      error = errorText(x);
    } finally {
      busy = false;
    }
  }

  async function refresh(r: Row) {
    r.status = 'pending';
    await post(`/api/admin/titles/${r.id}/refresh`).catch((x) => (error = errorText(x)));
    load();
  }

  // Фильм без файлов (импорт с сайтов, пустая карточка) - убрать из каталога.
  async function removeTitle(r: Row) {
    if (!(await ask(`Убрать «${r.title}» из каталога?`, 'Убрать', true))) return;
    try {
      await api(`/api/admin/titles/${r.id}`, { method: 'DELETE' });
      load();
    } catch (x) {
      error = errorText(x);
    }
  }
  const empty = $derived(rows.filter((r) => r.files === 0).length);
  async function removeEmpty() {
    if (!(await ask(`Убрать из каталога все фильмы без файлов (${empty})? Файлы на диске это не трогает.`, 'Убрать', true))) return;
    try {
      await api('/api/admin/titles/empty', { method: 'DELETE' });
      load();
    } catch (x) {
      error = errorText(x);
    }
  }

  async function toggleHidden(r: Row) {
    r.hidden = !r.hidden;
    await patch(`/api/admin/titles/${r.id}`, { hidden: r.hidden }).catch(() => (r.hidden = !r.hidden));
  }
</script>

<div class="bar">
  <div class="search input">
    <Icon name="search" size={16} />
    <input bind:value={q} placeholder="Название или путь к файлу" aria-label="Поиск по каталогу" />
  </div>
  <select class="input sel" bind:value={filter} aria-label="Фильтр">
    <option value="">Все</option>
    <option value="none">Описание не найдено</option>
    <option value="pending">Ищем описание</option>
    <option value="manual">Выбрано вручную</option>
    <option value="hidden">Скрытые</option>
  </select>
  {#if empty}<button class="btn sm danger" onclick={removeEmpty}><Icon name="trash" size={14} /> Убрать все без файлов ({empty})</button>{/if}
</div>
{#if error}<p class="error">{error}</p>{/if}

<ul class="list">
  {#each rows as r (r.id)}
    <li class:dim={r.hidden}>
      <a class="thumb" href="/title/{r.id}" style:background={r.poster ? undefined : placeholder(r.title)}>
        {#if r.poster}<img src={r.poster} alt="" loading="lazy" />{/if}
      </a>
      <div class="info">
        <b>{r.title}{r.year ? ` (${r.year})` : ''}</b>
        <span class="muted">{kindLabel(r.kind)} · {r.files} {r.kind === 'series' ? 'сер.' : 'файл.'} · <span class="st {r.status}">{STATUS[r.status]}</span>{r.hidden ? ' · скрыт' : ''}</span>
        {#if r.path}<span class="faint path" title={r.path}>{r.path}</span>{/if}
      </div>
      <div class="acts">
        <button class="btn sm" onclick={() => openMatch(r)}><Icon name="search" size={14} /> Найти</button>
        <button class="btn sm icon" aria-label="Изменить" title="Изменить" onclick={() => openEdit(r)}><Icon name="edit" size={15} /></button>
        <button class="btn sm icon" aria-label="Обновить описание" title="Обновить описание" onclick={() => refresh(r)}><Icon name="refresh" size={15} /></button>
        <button class="btn sm icon" aria-label={r.hidden ? 'Показать' : 'Скрыть'} title={r.hidden ? 'Показать зрителям' : 'Скрыть от зрителей'} onclick={() => toggleHidden(r)}>
          <Icon name={r.hidden ? 'eyeoff' : 'eye'} size={15} />
        </button>
        {#if r.files === 0}<button class="btn sm icon danger" aria-label="Убрать из каталога" title="Убрать из каталога" onclick={() => removeTitle(r)}><Icon name="trash" size={15} /></button>{/if}
      </div>
    </li>
  {:else}
    <li class="empty">Ничего нет</li>
  {/each}
</ul>

<Modal bind:open={editOpen} title="Изменить">
  <form class="form" onsubmit={saveEdit}>
    <label class="field"><span>Название</span><input class="input" bind:value={form.title} required /></label>
    <div class="two">
      <label class="field"><span>Год</span><input class="input" bind:value={form.year} inputmode="numeric" maxlength="4" /></label>
      <label class="field">
        <span>Вид</span>
        <select class="input" bind:value={form.kind}><option value="movie">Фильм</option><option value="series">Сериал</option></select>
      </label>
    </div>
    <label class="field">
      <span>Возраст - для детских профилей (0+, 6+, 12+, 16+, 18+, PG-13…)</span>
      <input class="input" bind:value={form.age} maxlength="8" placeholder="Не указан" />
    </label>
    <label class="field"><span>Описание</span><textarea class="input" bind:value={form.overview}></textarea></label>
    <div class="two">
      <label class="btn sm file"><Icon name="image" size={15} /> Свой постер<input type="file" accept="image/jpeg,image/png,image/webp" onchange={(e) => upload('poster', e)} /></label>
      <label class="btn sm file"><Icon name="image" size={15} /> Свой фон<input type="file" accept="image/jpeg,image/png,image/webp" onchange={(e) => upload('backdrop', e)} /></label>
    </div>
    <label class="check"><input type="checkbox" bind:checked={form.hidden} /> Скрыть от зрителей</label>
    <p class="muted small">После правки описание не будет обновляться само.</p>
    <button class="btn primary">Сохранить</button>
  </form>
</Modal>

<Modal bind:open={matchOpen} title="Найти описание" wide>
  <form class="sform" onsubmit={search}>
    <input class="input" bind:value={mq} placeholder="Название (лучше английское)" />
    <input class="input year" bind:value={my} placeholder="Год" inputmode="numeric" maxlength="4" />
    <button class="btn primary" disabled={busy}><Icon name="search" size={15} /> Искать</button>
  </form>
  {#if matching?.path}<p class="faint small path">{matching.path}</p>{/if}
  {#if busy && !cands}<p class="muted">Ищем…</p>{/if}
  {#if cands}
    <div class="cands">
      {#each cands as c (c.id)}
        <button class="cand" onclick={() => choose(c)} disabled={busy}>
          <span class="cp" style:background={c.poster ? undefined : placeholder(c.title)}>{#if c.poster}<img src={c.poster} alt="" loading="lazy" />{/if}</span>
          <span class="ct">
            <b>{c.title}</b>
            <span class="muted">{c.year ?? ''} · {c.series ? 'Сериал' : 'Фильм'}{c.original && c.original !== c.title ? ` · ${c.original}` : ''}</span>
            {#if c.overview}<span class="faint ov">{c.overview}</span>{/if}
          </span>
        </button>
      {:else}
        <p class="muted">Ничего не нашлось. Попробуйте английское название без года.</p>
      {/each}
    </div>
  {/if}
</Modal>

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-bottom: 14px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1 1 220px;
    min-width: 0;
    color: var(--muted);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: none;
  }
  .sel {
    width: auto;
    max-width: 100%;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--surface);
  }
  .list li.dim {
    opacity: 0.55;
  }
  .thumb {
    width: 44px;
    aspect-ratio: 2 / 3;
    border-radius: 6px;
    overflow: hidden;
    flex: none;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .info {
    flex: 1;
    min-width: 0;
    display: grid;
    font-size: 13px;
  }
  .info b {
    font-size: 14px;
  }
  .path {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .st.none {
    color: var(--warn);
  }
  .st.matched,
  .st.manual {
    color: var(--ok);
  }
  .acts {
    display: flex;
    gap: 6px;
  }
  .form {
    display: grid;
    gap: 14px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .file input {
    display: none;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 14px;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .form .btn.primary {
    justify-self: start;
  }
  .sform {
    display: flex;
    gap: 10px;
    margin-bottom: 8px;
  }
  .sform .year {
    width: 90px;
  }
  .cands {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 10px;
    margin-top: 12px;
  }
  .cand {
    display: flex;
    gap: 12px;
    padding: 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--line);
    background: var(--surface);
    text-align: left;
    cursor: pointer;
  }
  .cand:hover {
    border-color: var(--accent-2);
  }
  .cp {
    width: 60px;
    aspect-ratio: 2 / 3;
    border-radius: 6px;
    overflow: hidden;
    flex: none;
  }
  .cp img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .ct {
    display: grid;
    align-content: start;
    gap: 2px;
    font-size: 13px;
    min-width: 0;
  }
  .ov {
    font-size: 12px;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  @media (max-width: 640px) {
    .list li {
      flex-wrap: wrap;
    }
    .acts {
      width: 100%;
      justify-content: flex-end;
    }
  }
</style>
