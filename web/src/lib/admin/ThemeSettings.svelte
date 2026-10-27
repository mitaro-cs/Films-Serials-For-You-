<script lang="ts">
  import { api, errorText } from '$lib/api';
  import { toast } from '$lib/ui/toast.svelte';

  // Тема недели на главной: сама по календарю, одна из готовых или своя (название + жанр или режиссёр).
  type Choice = { mode: string; title: string; genre: string; director: string };
  let data = $state<{ choice: Choice; themes: { key: string; title: string; subtitle: string }[]; auto: string } | null>(null);
  let error = $state('');
  let busy = $state(false);

  $effect(() => {
    api<NonNullable<typeof data>>('/api/admin/theme').then((d) => (data = d), (e) => (error = errorText(e)));
  });

  async function save(e?: SubmitEvent) {
    e?.preventDefault();
    if (!data) return;
    busy = true;
    error = '';
    try {
      await api('/api/admin/theme', { method: 'PUT', json: data.choice });
      toast('Тема недели сохранена');
    } catch (x) {
      error = errorText(x);
    } finally {
      busy = false;
    }
  }
</script>

<section class="card pane">
  <h3>Тема недели</h3>
  {#if data}
    <p class="muted hint">Ряд на главной и пара кадров в герое. Сейчас по календарю: «{data.auto}».</p>
    <label class="field">
      <span>Тема</span>
      <select class="input" bind:value={data.choice.mode} onchange={() => data && data.choice.mode !== 'custom' && save()}>
        <option value="auto">Сама по неделям</option>
        {#each data.themes as t (t.key)}<option value={t.key}>{t.title} - {t.subtitle}</option>{/each}
        <option value="custom">Своя тема…</option>
      </select>
    </label>
    {#if data.choice.mode === 'custom'}
      <form class="custom" onsubmit={save}>
        <label class="field"><span>Название</span><input class="input" bind:value={data.choice.title} maxlength="60" placeholder="Например, «Неделя Спилберга»" required /></label>
        <label class="field"><span>Режиссёр</span><input class="input" bind:value={data.choice.director} maxlength="80" placeholder="Стивен Спилберг" /></label>
        <label class="field"><span>Или жанр</span><input class="input" bind:value={data.choice.genre} maxlength="40" placeholder="Фантастика" /></label>
        <button class="btn" disabled={busy}>Сохранить тему</button>
      </form>
    {/if}
  {/if}
  {#if error}<p class="error">{error}</p>{/if}
</section>

<style>
  /* Как у соседних карточек «Настроек». */
  .pane {
    display: grid;
    gap: 14px;
    padding: 18px 20px;
    margin-bottom: 14px;
  }
  h3 {
    font-size: 16px;
  }
  .hint {
    margin: -4px 0 0;
    font-size: 12.5px;
  }
  .custom {
    display: grid;
    gap: 12px;
  }
  .custom .btn {
    justify-self: start;
  }
</style>
