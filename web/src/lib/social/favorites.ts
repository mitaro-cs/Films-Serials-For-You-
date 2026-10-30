import { errorText, put } from '$lib/api';
import { session } from '$lib/session.svelte';
import { toast } from '$lib/ui/toast.svelte';
import { extChanged } from '$lib/discover/sheet.svelte';

// Избранное: «t:12» - с сервера, «x:tt…» - из Обзора.
// Сколько уже любимых этого вида - знает сервер (ответит понятной ошибкой).
export const MAX_FAVORITES = 4;
/** Любимый режиссёр в личном профиле - один; «нравится» можно ставить любым актёрам и режиссёрам. */
export const MAX_DIRECTORS = 1;

export const isFavorite = (key: string) => !!session.me?.favorites?.includes(key);

export async function toggleFavorite(key: string) {
  const me = session.me;
  if (!me) return;
  const value = !isFavorite(key);
  const before = me.favorites ?? [];
  me.favorites = value ? [...before, key] : before.filter((k) => k !== key);
  try {
    const r = await put<{ favorites: string[] }>('/api/me/favorites', { key, value });
    me.favorites = r.favorites;
    extChanged();
    toast(value ? 'Добавили в любимые' : 'Убрали из любимых');
  } catch (x) {
    me.favorites = before;
    toast(errorText(x));
  }
}
