<script lang="ts">
  import { del, errorText, patch, post, put } from '$lib/api';
  import { ago } from '$lib/fmt';
  import type { Review, Reviews } from '$lib/types';
  import Avatar from '$lib/ui/Avatar.svelte';
  import { ask } from '$lib/ui/dialogs.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import StarInput from './StarInput.svelte';
  import StarsShow from './StarsShow.svelte';

  // Отзывы и комментарии к фильму: свой - сверху формы, карточки - в две колонки (как на
  // референсе). Изменения - сразу на экране, сервер - следом. `base` - фильм с сервиса
  // (`/api/titles/12`) или из ленты (`/api/discover/title/tt…`): отзывы у них общие.
  let { base, data = $bindable(), manage = false }: { base: string; data: Reviews; manage?: boolean } = $props();

  let body = $state('');
  let stars = $state<number | null>(null);
  let busy = $state(false);
  let error = $state('');
  let editing = $state<number | null>(null);
  let editBody = $state('');
  let editStars = $state<number | null>(null);
  let showAll = $state(false);
  let textarea = $state<HTMLTextAreaElement>();

  const items = $derived(showAll ? data.items : data.items.slice(0, 6));

  export function focus() {
    textarea?.focus();
  }

  async function send(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      const r = await post<Review>(`${base}/reviews`, { body, stars });
      data.items = [r, ...data.items];
      data.total += 1;
      if (stars) recount();
      body = '';
      stars = null;
    } catch (x) {
      error = errorText(x);
    } finally {
      busy = false;
    }
  }

  /** Средняя - по последней оценке каждого человека (как на сервере). */
  function recount() {
    const last = new Map<number, number>();
    for (const r of [...data.items].reverse()) if (r.stars) last.set(r.author.id, r.stars);
    const v = [...last.values()];
    data.votes = v.length;
    data.average = v.length ? v.reduce((a, b) => a + b, 0) / v.length / 2 : null;
  }

  async function like(r: Review) {
    const before = { liked: r.liked, likes: r.likes };
    r.liked = !r.liked;
    r.likes += r.liked ? 1 : -1;
    try {
      await put(`/api/reviews/${r.id}/like`, { value: r.liked });
    } catch {
      Object.assign(r, before);
    }
  }

  function startEdit(r: Review) {
    editing = r.id;
    editBody = r.body;
    editStars = r.stars;
  }

  async function saveEdit(r: Review) {
    try {
      const n = await patch<Review>(`/api/reviews/${r.id}`, { body: editBody, stars: editStars });
      Object.assign(r, n);
      editing = null;
      recount();
    } catch (x) {
      error = errorText(x);
    }
  }

  async function remove(r: Review) {
    if (!(await ask(r.mine ? 'Удалить ваш отзыв?' : `Удалить отзыв «${r.author.name}»?`, 'Удалить', true))) return;
    try {
      await del(`/api/reviews/${r.id}`);
      data.items = data.items.filter((x) => x.id !== r.id);
      data.total -= 1;
      recount();
    } catch (x) {
      error = errorText(x);
    }
  }
</script>

{#if data.canWrite}
  <form class="compose" onsubmit={send}>
    <div class="top">
      <span class="muted">Ваша оценка</span>
      <StarInput value={stars} onchange={(v) => (stars = v)} size={24} />
    </div>
    <textarea bind:this={textarea} class="input" bind:value={body} maxlength="4000" rows="3" placeholder="Что думаете? Спойлеры - по-честному предупреждайте"></textarea>
    {#if error}<p class="error">{error}</p>{/if}
    <div class="row">
      <span class="faint small">{body.length > 3500 ? `${4000 - body.length} знаков осталось` : ''}</span>
      <button class="btn primary sm" disabled={busy || (!body.trim() && !stars)}>{busy ? 'Отправляем…' : 'Опубликовать'}</button>
    </div>
  </form>
{/if}

{#if data.items.length}
  <ul class="list">
    {#each items as r (r.id)}
      <li class="card review">
        <header>
          <a href="/people/{r.author.id}" class="who">
            <Avatar avatar={r.author.avatar} photo={r.author.photo} size={36} />
            <span><b>{r.author.name}</b><small class="faint">{ago(r.createdAt)}{r.editedAt ? ' · изменено' : ''}</small></span>
          </a>
          {#if r.stars}<span class="score"><StarsShow value={r.stars} /> {String(r.stars / 2).replace('.', ',')}</span>{/if}
        </header>
        {#if editing === r.id}
          <div class="edit">
            <StarInput value={editStars} onchange={(v) => (editStars = v)} size={22} />
            <textarea class="input" bind:value={editBody} maxlength="4000" rows="3"></textarea>
            <div class="row">
              <button class="btn sm ghost" onclick={() => (editing = null)}>Отмена</button>
              <button class="btn sm primary" onclick={() => saveEdit(r)}>Сохранить</button>
            </div>
          </div>
        {:else if r.body}
          <p class="text">{r.body}</p>
        {/if}
        <footer>
          <button class="act" class:on={r.liked} aria-pressed={r.liked} aria-label="Нравится" disabled={!data.canWrite} onclick={() => like(r)}>
            <Icon name="heart" size={15} /> {r.likes || ''}
          </button>
          <span class="grow"></span>
          {#if r.mine && editing !== r.id}<button class="act" onclick={() => startEdit(r)}><Icon name="edit" size={14} /> Изменить</button>{/if}
          {#if r.mine || manage}<button class="act" aria-label="Удалить отзыв" onclick={() => remove(r)}><Icon name="trash" size={14} /></button>{/if}
        </footer>
      </li>
    {/each}
  </ul>
  {#if data.items.length > 6 && !showAll}
    <button class="btn ghost sm more" onclick={() => (showAll = true)}>Все отзывы ({data.items.length})</button>
  {/if}
{:else}
  <p class="muted none">Отзывов пока нет - будьте первым.</p>
{/if}

<style>
  .compose {
    display: grid;
    gap: 10px;
    margin-bottom: 18px;
  }
  .top,
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .small {
    font-size: 12px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(320px, 100%), 1fr));
    gap: 12px;
  }
  .review {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 18px 12px;
    background: #232325;
    border-color: transparent;
    animation: rise 0.35s cubic-bezier(0.2, 0.8, 0.2, 1) both;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .who span {
    display: grid;
    min-width: 0;
  }
  .who b {
    font-size: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who small {
    font-size: 11.5px;
  }
  .score {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
    font-size: 12.5px;
    font-weight: 700;
  }
  .text {
    margin: 0;
    font-size: 13.5px;
    line-height: 1.55;
    color: #d4d4d8;
    white-space: pre-line;
    overflow-wrap: anywhere;
  }
  .edit {
    display: grid;
    gap: 8px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: auto;
  }
  .grow {
    flex: 1;
  }
  .act {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 8px;
    border: 0;
    border-radius: 99px;
    background: none;
    color: var(--muted);
    font-size: 12.5px;
    cursor: pointer;
    transition:
      color 0.15s,
      background 0.15s,
      transform 0.15s;
  }
  .act:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-2);
  }
  .act:active:not(:disabled) {
    transform: scale(0.92);
  }
  .act.on {
    color: #ff5c7a;
  }
  .more {
    margin-top: 12px;
  }
  .none {
    margin: 0;
  }
  :global(.no-motion) .review {
    animation: none;
  }
</style>
