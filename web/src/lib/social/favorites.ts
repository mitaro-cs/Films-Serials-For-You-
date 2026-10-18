import { errorText, put } from '$lib/api';
import { session } from '$lib/session.svelte';
import { toast } from '$lib/ui/toast.svelte';

// Пять любимых фильмов, как на IMDb: «t:12» — с сервиса, «x:tt…» — из Обзора.
export const MAX_FAVORITES = 5;

export const isFavorite = (key: string) => !!session.me?.favorites?.includes(key);

export async function toggleFavorite(key: string) {
  const me = session.me;
  if (!me) return;
  const value = !isFavorite(key);
  if (value && (me.favorites?.length ?? 0) >= MAX_FAVORITES) {
    toast(`Любимых — не больше ${MAX_FAVORITES}: уберите один в профиле`);
    return;
  }
  const before = me.favorites ?? [];
  me.favorites = value ? [...before, key] : before.filter((k) => k !== key);
  try {
    const r = await put<{ favorites: string[] }>('/api/me/favorites', { key, value });
    me.favorites = r.favorites;
    toast(value ? `В любимых: ${r.favorites.length} из ${MAX_FAVORITES}` : 'Убрали из любимых');
  } catch (x) {
    me.favorites = before;
    toast(errorText(x));
  }
}
