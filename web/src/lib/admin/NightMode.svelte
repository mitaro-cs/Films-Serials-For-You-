<script lang="ts">
  import { post, errorText } from '$lib/api';
  import { session } from '$lib/session.svelte';
  type Result = { ok: boolean; error?: string; supported?: number; changed?: number; skipped?: number; lights?: Result };
  let busy = $state(false);
  let lights = $state(false);
  let supported = $state(0);
  let message = $state('');
  let error = $state('');
  async function action(action: 'probe' | 'off' | 'restore') {
    busy = true; message = error = '';
    try {
      const result = await post<Result>('/api/host/night', { action, lights });
      if (!result.ok) { error = result.error || 'Mystic Light не смог выполнить команду.'; return; }
      if (action === 'probe') {
        supported = result.supported ?? 0;
        lights = supported > 0;
        message = supported ? `Доступно зон подсветки: ${supported}.` : 'Управляемая подсветка не обнаружена. Экран можно погасить отдельно.';
      } else if (action === 'restore') message = `Яркость восстановлена для зон: ${result.changed ?? 0}.`;
      else {
        message = 'Через 3 секунды экран погаснет. Nova продолжит работать в трее.';
        if (result.lights && !result.lights.ok) error = result.lights.error || 'Часть подсветки не отключилась. Восстановите её кнопкой ниже.';
      }
    } catch (e) { error = errorText(e); }
    finally { busy = false; }
  }
</script>

{#if session.me?.window}
  <section class="card pane" aria-labelledby="night-title">
    <h3 id="night-title">Ночной режим хоста</h3>
    <p class="muted">Экран гаснет, а компьютер продолжает раздавать видео. Чтобы включить экран, нажмите клавишу или пошевелите мышью.</p>
    <div class="night-actions">
      <button class="btn primary" disabled={busy} onclick={() => action('off')}>Погасить экран</button>
      <button class="btn" disabled={busy} onclick={() => action('probe')}>Проверить подсветку MSI</button>
      <button class="btn" disabled={busy} onclick={() => action('restore')}>Вернуть подсветку</button>
    </div>
    {#if supported > 0}<label class="night-check"><input type="checkbox" bind:checked={lights} disabled={busy} /> Также погасить поддерживаемую подсветку MSI</label>{/if}
    <p class="muted small">Для подсветки нужен работающий Mystic Light. Предыдущая яркость сохраняется. Индикаторы питания и неподдерживаемые устройства остаются включёнными. Режим доступен в Windows.</p>
    {#if busy}<p class="muted" role="status">Выполняется…</p>{/if}
    {#if message}<p role="status">{message}</p>{/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </section>
{/if}

<style>
  .pane { padding:24px; margin-bottom:20px }
  h3 { margin:0 0 12px }
  p { line-height:1.6 }
  .night-actions {display:flex;flex-wrap:wrap;gap:10px;margin:18px 0}
  .night-check {display:flex;align-items:center;gap:10px;margin:16px 0}
  .small {font-size:12px}
</style>
