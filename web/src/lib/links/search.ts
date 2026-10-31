import { api } from '$lib/api';
import type { FoundVideo } from '$lib/types';

export const sourceName = (site: string) => ({lordfilm:'LordFilm',rutube:'Rutube',vk:'VK Видео',archive:'Internet Archive',peertube:'PeerTube',commons:'Wikimedia Commons',dailymotion:'Dailymotion'}[site] ?? site);
const sources = ['lordfilm','rutube','vk','archive','peertube','commons'];

/** Publish each provider independently: a slow server must not hide ready results. */
export async function searchVideos(params: URLSearchParams, publish: (items: FoundVideo[]) => void, signal?: AbortSignal, otherSourcesOnly = false) {
  const found: FoundVideo[] = [];
  const seen = new Set<string>();
  let success = 0;
  await Promise.all(sources.filter(source => !otherSourcesOnly || source !== 'lordfilm').map(async provider => {
    const query = new URLSearchParams(params); query.set('provider',provider);
    try {
      const items = await api<FoundVideo[]>('/api/links/search?'+query, {signal});
      success++;
      if (signal?.aborted) return;
      for(const item of items) if(!seen.has(item.url)) {seen.add(item.url);found.push(item)}
      publish([...found]);
    } catch { /* Other providers remain usable. */ }
  }));
  if(!success && !signal?.aborted) throw new Error('Источники видео сейчас не отвечают');
  return found;
}

export async function searchLordfilm(params: URLSearchParams, signal?: AbortSignal) {
  const query = new URLSearchParams(params);
  query.set('provider', 'lordfilm');
  return api<FoundVideo[]>('/api/links/search?' + query, {signal});
}

export function isLordfilm(url: string) {
  try { return ['sd.lordfilm5.pro', 'sg.lordfilm5.pro'].includes(new URL(url).hostname); } catch { return false; }
}
