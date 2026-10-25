// Страница человека кино внутри Nova (фото и фильмография с IMDb): актёр - его роли, режиссёр и
// создатель - что снял. Id IMDb («nm…») - если он есть в описании (без поиска по имени).
export function personHref(p: { name: string; job?: string; id?: unknown }): string {
  const q = new URLSearchParams({ name: p.name, role: p.job === 'actor' ? 'actor' : 'director' });
  if (typeof p.id === 'string' && p.id.startsWith('nm')) q.set('id', p.id);
  return `/person?${q}`;
}
