<script lang="ts">
  import { api, del, errorText, post } from '$lib/api';
  import { openExt } from '$lib/discover/sheet.svelte';
  import { day } from '$lib/library';
  import { badges } from '$lib/social/badges.svelte';
  import type { ExtCard } from '$lib/types';
  import Icon from '$lib/ui/Icon.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  // Что зрители просят добавить (кнопка «Хочу на сервисе» в Обзоре). Добавили фильм с тем же
  // IMDb - просьба уходит сама; «Не будем» - убрать вручную.
  interface Wish {
    card: ExtCard;
    count: number;
    people: string;
    at: number;
  }
  let wishes = $state<Wish[] | null>(null);
  let error = $state('');

  function load() {
    api<Wish[]>('/api/admin/wishes').then(
      (r) => {
        wishes = r;
        badges.wishes = r.length;
      },
      (e) => (error = errorText(e)),
    );
  }
  $effect(load);

  async function dismiss(w: Wish) {
    wishes = wishes?.filter((x) => x !== w) ?? null;
    badges.wishes = Math.max(0, badges.wishes - 1);
    await del(`/api/admin/wishes/${w.card.id}`).catch(load);
  }

  async function refresh() {
    try {
      await post('/api/admin/discover/refresh');
      toast('Подборки IMDb обновятся через минуту');
    } catch (x) {
      toast(errorText(x));
    }
  }
</script>

<section class="card pane">
  <div class="top">
    <h3>Просьбы зрителей</h3>
    <button class="btn sm ghost" onclick={refresh}><Icon name="refresh" size={14} /> Обновить ленту IMDb</button>
  </div>
  {#if error}
    <p class="error">{error}</p>
  {:else if wishes === null}
    <div class="ph skeleton"></div>
  {:else if wishes.length}
    <ul>
      {#each wishes as w (w.card.id)}
        <li>
          <button class="art" onclick={() => openExt(w.card)} aria-label="Открыть «{w.card.title}»">
            {#if w.card.poster}<img src={w.card.poster} alt="" loading="lazy" decoding="async" />{/if}
          </button>
          <div class="info">
            <b>{w.card.title}{#if w.card.year}<span class="muted"> · {w.card.year}</span>{/if}</b>
            <span class="muted">Просят: {w.people}{w.count > 1 ? ` (${w.count})` : ''} · {day(w.at)}</span>
          </div>
          <button class="btn sm ghost" onclick={() => dismiss(w)}>Не будем</button>
        </li>
      {/each}
    </ul>
    <p class="faint note">Чтобы просьба ушла сама, добавьте фильм в папку - Nova узнает его по IMDb.</p>
  {:else}
    <p class="muted">Просьб нет. Зрители могут попросить фильм в Обзоре - кнопкой «Хочу на сервисе».</p>
  {/if}
</section>

<style>
  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }
  .art {
    flex: none;
    width: 46px;
    aspect-ratio: 2 / 3;
    padding: 0;
    border: 0;
    border-radius: 6px;
    overflow: hidden;
    background: var(--surface-2);
    cursor: pointer;
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .info {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 2px;
  }
  .info b,
  .info span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .info span {
    font-size: 13px;
  }
  .ph {
    height: 120px;
    border-radius: 12px;
  }
  .note {
    font-size: 13px;
    margin: 0;
  }
</style>
