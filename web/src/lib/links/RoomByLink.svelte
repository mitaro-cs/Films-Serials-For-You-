<script lang="ts">
  import { goto } from '$app/navigation';
  import { errorText, post } from '$lib/api';
  import Modal from '$lib/ui/Modal.svelte';
  import Icon from '$lib/ui/Icon.svelte';

  // «Комната по ссылке»: вставили ссылку на видео - комната готова, можно звать друзей. Ссылка - любая из
  // поддерживаемых: Rutube, VK Видео, YouTube, прямой .mp4/.webm/.m3u8 и сайты, которые хост добавил в
  // «Управление → Настройки → Свои сайты». Открывается событием `nova-room-link` (меню профиля).
  let open = $state(false);
  let url = $state('');
  let label = $state('');
  let busy = $state(false);
  let error = $state('');

  $effect(() => {
    const show = () => {
      url = '';
      label = '';
      error = '';
      open = true;
    };
    window.addEventListener('nova-room-link', show);
    return () => window.removeEventListener('nova-room-link', show);
  });

  async function create(e: SubmitEvent) {
    e.preventDefault();
    if (busy) return;
    if (!url.trim()) {
      error = 'Вставьте ссылку на видео';
      return;
    }
    busy = true;
    error = '';
    try {
      const link = await post<{ id: number }>('/api/links', { url: url.trim(), label: label.trim() || undefined });
      const room = await post<{ id: string }>('/api/rooms', { linkId: link.id, position: 0 });
      open = false;
      await goto(`/room/${room.id}`);
    } catch (x) {
      error = errorText(x);
    } finally {
      busy = false;
    }
  }
</script>

<Modal bind:open title="Комната по ссылке">
  <form onsubmit={create}>
    <p class="muted">Вставьте ссылку на видео - откроется комната, куда можно позвать друзей: видео идёт у всех одинаково, есть чат и голос.</p>
    <label class="field">
      <span>Ссылка</span>
      <input class="input" type="url" bind:value={url} placeholder="https://rutube.ru/video/…" autocomplete="off" />
    </label>
    <label class="field">
      <span>Название (необязательно)</span>
      <input class="input" bind:value={label} maxlength="120" placeholder="Например, «Фильм на пятницу»" />
    </label>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <p class="hint">Подходят Rutube, VK Видео, YouTube, прямые файлы .mp4, .webm, .m3u8 и сайты из списка «Свои сайты» у хозяина Nova.</p>
    <button class="btn primary" disabled={busy}><Icon name="party" size={16} /> {busy ? 'Создаём комнату…' : 'Создать комнату'}</button>
  </form>
</Modal>

<style>
  form {
    display: grid;
    gap: 12px;
  }
  .field {
    display: grid;
    gap: 6px;
    font-size: 13px;
    color: var(--muted);
  }
  .error {
    margin: 0;
    color: #ff8a8a;
    font-size: 13px;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
    line-height: 1.5;
  }
  p.muted {
    margin: 0;
    font-size: 14px;
  }
</style>
