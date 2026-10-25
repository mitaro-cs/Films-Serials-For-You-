import { api } from '$lib/api';
import { session } from '$lib/session.svelte';
import { extSheet } from './sheet.svelte';
import type { ExtDetail } from '$lib/types';
const cache = new Map<string, { at: number; data: ExtDetail }>();
const pending = new Map<string, Promise<ExtDetail>>();
const key = (id: string) => `${session.me?.user.profile?.id}:${extSheet.version}:${id}`;
export const peekDetail = (id: string) => cache.get(key(id))?.data ?? null;
export function getDetail(id: string): Promise<ExtDetail> {
  const k = key(id), saved = cache.get(k);
  if (saved && Date.now() - saved.at < 60_000 && !saved.data.refreshing) return Promise.resolve(saved.data);
  const running = pending.get(k); if (running) return running;
  const task = api<ExtDetail>(`/api/discover/title/${id}`).then((data) => {
    if (cache.size >= 30) cache.delete(cache.keys().next().value!);
    cache.set(k, { at: Date.now(), data }); return data;
  }).finally(() => pending.delete(k));
  pending.set(k, task); return task;
}
