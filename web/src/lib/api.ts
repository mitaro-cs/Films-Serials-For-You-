import { goto } from '$app/navigation';
import { net } from './session.svelte';

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
  ) {
    super(message);
  }
}

const AUTH_PAGES = ['/login', '/join', '/register'];

export async function api<T = unknown>(path: string, init: RequestInit & { json?: unknown } = {}): Promise<T> {
  const headers = new Headers(init.headers);
  const method = (init.method ?? 'GET').toUpperCase();
  if (method !== 'GET' && method !== 'HEAD') headers.set('x-fsfy', '1');
  let body = init.body;
  if (init.json !== undefined) {
    headers.set('content-type', 'application/json');
    body = JSON.stringify(init.json);
  }
  let res: Response;
  try {
    res = await fetch(path, { ...init, method, headers, body, credentials: 'same-origin' });
  } catch {
    net.down = true;
    throw new ApiError(0, 'offline', 'Нет связи с сервером');
  }
  // Ответ не от нашего сервера (нет метки) — это страница туннеля: компьютер хоста выключен.
  if (!res.headers.get('x-fsfy') && res.status >= 500) {
    net.down = true;
    throw new ApiError(res.status, 'offline', 'Компьютер хоста сейчас выключен');
  }
  net.down = false;
  const data = res.headers.get('content-type')?.includes('json') ? await res.json() : null;
  if (!res.ok) {
    if (res.status === 401 && !AUTH_PAGES.some((p) => location.pathname.startsWith(p))) {
      const next = location.pathname + location.search;
      goto(`/login?next=${encodeURIComponent(next)}`, { replaceState: true });
    }
    throw new ApiError(res.status, data?.error ?? 'error', data?.message ?? `Ошибка ${res.status}`);
  }
  return data as T;
}

export const post = <T = unknown>(path: string, json?: unknown) => api<T>(path, { method: 'POST', json: json ?? {} });
export const put = <T = unknown>(path: string, json?: unknown) => api<T>(path, { method: 'PUT', json: json ?? {} });
export const patch = <T = unknown>(path: string, json?: unknown) => api<T>(path, { method: 'PATCH', json: json ?? {} });
export const del = <T = unknown>(path: string) => api<T>(path, { method: 'DELETE' });

export function errorText(e: unknown): string {
  return e instanceof ApiError ? e.message : 'Что-то пошло не так';
}
