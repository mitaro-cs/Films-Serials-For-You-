<script lang="ts">
  import { errorText, post } from '$lib/api';
  import { size } from '$lib/fmt';
  import Icon from '$lib/ui/Icon.svelte';
  import type { Status } from './types';

  // Самый простой путь добавить фильмы: своя папка (положил — появилось), загрузка из
  // браузера с любого устройства, перетаскивание в окно приложения хоста.
  let { status, window: inWindow, reload }: { status: Status; window: boolean; reload: () => void } = $props();

  const VIDEO = /\.(mp4|m4v|mkv|webm|mov|avi|wmv|ts|m2ts|mpg|mpeg|flv|ogv|3gp|srt|ass|ssa|vtt|sub)$/i;
  interface Up {
    id: number;
    file: File;
    folder: string;
    done: number;
    state: 'wait' | 'up' | 'ok' | 'err';
    error?: string;
    xhr?: XMLHttpRequest;
  }
  let queue = $state<Up[]>([]);
  let over = $state(false);
  let error = $state('');
  let n = 0;

  const EPISODE = /s\d{1,2}[ ._-]*e\d{1,3}|\d{1,2}x\d{2,3}|сезон|серия/i;

  /** Одиночный файл: фильм — в свою папку («Начало.2010.1080p»), серия с «S01E02» — как есть. */
  const folderOf = (f: File) =>
    EPISODE.test(f.name) ? '' : f.name.replace(/\.[^.]+$/, '').replace(/\.(ru|en|rus|eng)$/i, '');

  /** Файл и папка, где он лежал: «Сериал/Сезон 1/01.mkv» → «Сериал/Сезон 1». */
  type Picked = { file: File; dir: string | null };

  function add(files: Picked[]) {
    const ok = files.filter((p) => VIDEO.test(p.file.name));
    error = ok.length < files.length ? 'Пропущено то, что не видео и не субтитры' : '';
    // Серии по порядку: так сериал появляется целиком и сразу по номерам.
    ok.sort((a, b) => `${a.dir}/${a.file.name}`.localeCompare(`${b.dir}/${b.file.name}`, 'ru', { numeric: true }));
    queue = [
      ...queue,
      ...ok.map((p) => ({ id: n++, file: p.file, folder: p.dir ?? folderOf(p.file), done: 0, state: 'wait' as const })),
    ];
    pump();
  }

  const loose = (list: FileList | null): Picked[] =>
    [...(list ?? [])].map((file) => {
      const rel = file.webkitRelativePath;
      return { file, dir: rel ? rel.split('/').slice(0, -1).join('/') : null };
    });

  // Перетащили папку — обходим её целиком (с подпапками сезонов).
  async function dropped(dt: DataTransfer) {
    const entries = [...dt.items].map((i) => i.webkitGetAsEntry?.()).filter((e): e is FileSystemEntry => !!e);
    if (!entries.some((e) => e.isDirectory)) return add(loose(dt.files));
    const out: Picked[] = [];
    const walk = async (e: FileSystemEntry, dir: string): Promise<void> => {
      if (e.isFile) {
        const file = await new Promise<File>((ok, no) => (e as FileSystemFileEntry).file(ok, no));
        out.push({ file, dir: dir || null });
        return;
      }
      const reader = (e as FileSystemDirectoryEntry).createReader();
      const path = dir ? `${dir}/${e.name}` : e.name;
      for (;;) {
        const batch = await new Promise<FileSystemEntry[]>((ok, no) => reader.readEntries(ok, no));
        if (!batch.length) break;
        for (const c of batch) await walk(c, path);
      }
    };
    for (const e of entries) await walk(e, '');
    add(out);
  }

  // По одному файлу за раз: так быстрее на медленном туннеле и понятнее прогресс.
  function pump() {
    if (queue.some((u) => u.state === 'up')) return;
    const u = queue.find((x) => x.state === 'wait');
    if (!u) return reload();
    u.state = 'up';
    const x = new XMLHttpRequest();
    u.xhr = x;
    const more = queue.some((q) => q !== u && q.state === 'wait') ? '1' : '0';
    x.open('PUT', `/api/admin/upload?${new URLSearchParams({ name: u.file.name, folder: u.folder, more })}`);
    x.setRequestHeader('x-fsfy', '1');
    x.upload.onprogress = (e) => (u.done = e.loaded);
    x.onload = () => {
      if (x.status >= 200 && x.status < 300) u.state = 'ok';
      else {
        u.state = 'err';
        try {
          u.error = JSON.parse(x.responseText).message;
        } catch {
          u.error = `Ошибка ${x.status}`;
        }
      }
      pump();
    };
    x.onerror = () => {
      u.state = 'err';
      u.error = 'Связь прервалась';
      pump();
    };
    x.send(u.file);
  }

  function cancel(u: Up) {
    u.xhr?.abort();
    queue = queue.filter((x) => x.id !== u.id);
    pump();
  }

  async function create() {
    try {
      await post('/api/admin/inbox');
      reload();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function reveal() {
    await post('/api/admin/inbox/reveal').catch((e) => (error = errorText(e)));
  }

  const imp = $derived(status.import);
  const pct = (u: Up) => Math.round((u.done / Math.max(1, u.file.size)) * 100);
</script>

<section class="card pane inbox" aria-labelledby="inbox-h">
  <h3 id="inbox-h"><Icon name="plus" size={18} /> Добавить фильмы</h3>

  {#if status.inbox}
    <div class="folder">
      <Icon name="folder" size={22} />
      <div>
        <b>Своя папка</b>
        <code>{status.inbox}</code>
        <small class="muted">Положите сюда фильмы и сериалы — появятся на сайте сами, за несколько секунд.</small>
      </div>
      {#if inWindow}<button class="btn sm" onclick={reveal}><Icon name="folder" size={14} /> Открыть</button>{/if}
    </div>
  {:else}
    <button class="btn" onclick={create}><Icon name="folder" size={16} /> Создать свою папку для фильмов</button>
  {/if}

  {#if inWindow}
    <p class="tip"><Icon name="info" size={15} /> Можно просто перетащить файлы или папки в это окно: папки подключатся как есть, файлы скопируются в свою папку.</p>
  {/if}

  <label
    class="drop"
    class:over
    ondragenter={(e) => (e.preventDefault(), (over = true))}
    ondragover={(e) => e.preventDefault()}
    ondragleave={() => (over = false)}
    ondrop={(e) => {
      e.preventDefault();
      over = false;
      if (e.dataTransfer) dropped(e.dataTransfer);
    }}
  >
    <Icon name="download" size={26} />
    <b>Перетащите видео или папку с сериалом</b>
    <small class="muted">или нажмите, чтобы выбрать файлы. С телефона и другого компьютера — тоже.</small>
    <input type="file" multiple accept="video/*,.mkv,.avi,.srt,.ass,.vtt" onchange={(e) => add(loose((e.currentTarget as HTMLInputElement).files))} hidden />
  </label>
  <label class="btn folder-pick">
    <Icon name="folder" size={16} /> Загрузить папку с сериалом
    <input type="file" webkitdirectory multiple onchange={(e) => add(loose((e.currentTarget as HTMLInputElement).files))} hidden />
  </label>
  {#if error}<p class="error">{error}</p>{/if}

  {#if queue.length}
    <ul class="ups">
      {#each queue as u (u.id)}
        <li>
          <div class="row">
            <span class="name">{u.file.name}</span>
            <span class="faint">
              {#if u.state === 'ok'}Готово{:else if u.state === 'err'}{u.error}{:else if u.state === 'up'}{pct(u)}% · {size(u.file.size)}{:else}В очереди{/if}
            </span>
            {#if u.state === 'up' || u.state === 'wait'}
              <button class="btn ghost sm icon" aria-label="Отменить" onclick={() => cancel(u)}><Icon name="close" size={14} /></button>
            {/if}
          </div>
          <span class="bar" class:ok={u.state === 'ok'} class:err={u.state === 'err'}><i style:width="{u.state === 'ok' ? 100 : pct(u)}%"></i></span>
        </li>
      {/each}
    </ul>
  {/if}

  {#if imp.running}
    <div class="ups">
      <div class="row">
        <span class="name">Копируем {imp.filesDone + 1} из {imp.filesTotal}: {imp.current}</span>
        <span class="faint">{Math.round((imp.bytesDone / Math.max(1, imp.bytesTotal)) * 100)}%</span>
      </div>
      <span class="bar"><i style:width="{(imp.bytesDone / Math.max(1, imp.bytesTotal)) * 100}%"></i></span>
    </div>
  {:else if imp.error}
    <p class="error">{imp.error}</p>
  {/if}
</section>

<style>
  .folder-pick {
    justify-self: start;
  }
  .inbox {
    display: grid;
    gap: 14px;
    padding: 20px;
    margin-bottom: 18px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
  }
  .folder {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  .folder > div {
    display: grid;
    gap: 3px;
    flex: 1;
    min-width: 0;
  }
  code {
    font-size: 12.5px;
    overflow-wrap: anywhere;
    color: var(--muted);
  }
  .tip {
    display: flex;
    gap: 8px;
    margin: 0;
    font-size: 13px;
    color: var(--muted);
  }
  .drop {
    display: grid;
    justify-items: center;
    gap: 6px;
    padding: 26px 16px;
    border: 1.5px dashed var(--line-strong);
    border-radius: 16px;
    text-align: center;
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s,
      transform 0.15s;
  }
  .drop:hover,
  .drop.over {
    border-color: #fff;
    background: rgba(255, 255, 255, 0.04);
  }
  .drop.over {
    transform: scale(1.01);
  }
  .ups {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bar {
    display: block;
    height: 4px;
    margin-top: 6px;
    border-radius: 2px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .bar i {
    display: block;
    height: 100%;
    background: #fff;
    transition: width 0.2s;
  }
  .bar.ok i {
    background: var(--ok);
  }
  .bar.err i {
    background: var(--danger);
  }
  p {
    margin: 0;
  }
</style>
