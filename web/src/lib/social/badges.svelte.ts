import { api } from '$lib/api';
import { goto } from '$app/navigation';
import { session } from '$lib/session.svelte';
import { toast } from '$lib/ui/toast.svelte';
import { noteWatchedAt } from '$lib/progressSync.svelte';

// Значки в шапке: просьбы дружбы, непрочитанные сообщения, просьбы добавить фильм (хосту). Опрос раз в 30 с, пока вкладка
// видна, и сразу при возвращении на неё.
export const badges = $state({ requests: 0, unread: 0, wishes: 0 });

let pending: Promise<void> | undefined;

export function refreshBadges(): Promise<void> {
  if (pending) return pending;
  pending = loadBadges().finally(() => { pending = undefined; });
  return pending;
}

async function loadBadges() {
  try {
    const { watchedAt, invitation, ...counts } = await api<{ requests: number; unread: number; wishes: number; watchedAt?: number; invitation?: {id: number; from: string; room: string} | null }>('/api/badges');
    if (invitation) {
      const key = `nova-invite:${session.me?.user.profile?.id ?? ''}`;
      let last = 0; try { last = Number(sessionStorage.getItem(key) ?? 0); } catch {}
      if (invitation.id > last) {
        try { sessionStorage.setItem(key, String(invitation.id)); } catch {}
        toast(`${invitation.from} приглашает смотреть вместе`, 15000, {label: 'Открыть', run: () => { void goto(`/room/${encodeURIComponent(invitation.room)}`); }});
      }
    }
    Object.assign(badges, counts);
    noteWatchedAt(watchedAt);
  } catch {
    /* сервер недоступен - значки подождут */
  }
}

export function watchBadges(): () => void {
  refreshBadges();
  const tick = () => document.visibilityState === 'visible' && refreshBadges();
  const timer = setInterval(tick, 5000);
  document.addEventListener('visibilitychange', tick);
  // Окно приложения могло быть видно, но без фокуса: вернулись к компьютеру - сразу сверить.
  window.addEventListener('focus', tick);
  return () => {
    window.removeEventListener('focus', tick);
    clearInterval(timer);
    document.removeEventListener('visibilitychange', tick);
  };
}
