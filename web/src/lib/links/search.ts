import { api } from '$lib/api';
import type { FoundVideo } from '$lib/types';

export const sourceName = (site: string) => ({rutube:'Rutube',vk:'VK Видео',archive:'Internet Archive',peertube:'PeerTube',commons:'Wikimedia Commons',dailymotion:'Dailymotion'}[site] ?? site);
const sources = ['rutube','vk','archive','peertube','commons'];

/** Publish each provider independently: a slow server must not hide ready results. */
export async function searchVideos(params: URLSearchParams, publish: (items: FoundVideo[]) => void, signal?: AbortSignal) {
  const found: FoundVideo[] = [];
  const seen = new Set<string>();
  let success = 0;
  await Promise.all(sources.map(async provider => {
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
