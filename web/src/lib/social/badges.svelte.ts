import { api } from '$lib/api';

// Значки в шапке: просьбы дружбы и непрочитанные сообщения. Опрос раз в 30 с, пока вкладка
// видна, и сразу при возвращении на неё.
export const badges = $state({ requests: 0, unread: 0 });

let timer: ReturnType<typeof setInterval> | undefined;

export async function refreshBadges() {
  try {
    Object.assign(badges, await api<{ requests: number; unread: number }>('/api/badges'));
  } catch {
    /* сервер недоступен — значки подождут */
  }
}

export function watchBadges(): () => void {
  refreshBadges();
  const tick = () => document.visibilityState === 'visible' && refreshBadges();
  timer = setInterval(tick, 30_000);
  document.addEventListener('visibilitychange', tick);
  return () => {
    clearInterval(timer);
    document.removeEventListener('visibilitychange', tick);
  };
}
