// Запросы ТВ-интерфейса. Без Svelte и без общего кода сайта: сборка идёт под Chrome 69 (Samsung
// Tizen 5.5, 2020), туда не должны попасть современные API.

export class ApiError extends Error {
  status: number;
  code: string;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

interface Init {
  method?: string;
  json?: unknown;
  signal?: AbortSignal;
}

export async function api<T = unknown>(path: string, init: Init = {}): Promise<T> {
  const method = init.method || 'GET';
  const headers: Record<string, string> = {};
  let body: string | undefined;
  if (method !== 'GET') headers['x-fsfy'] = '1';
  if (init.json !== undefined) {
    headers['content-type'] = 'application/json';
    body = JSON.stringify(init.json);
  }
  let res: Response;
  try {
    res = await fetch(path, { method, headers, body, credentials: 'same-origin', signal: init.signal });
  } catch (e) {
    if (init.signal && init.signal.aborted) throw e;
    throw new ApiError(0, 'offline', 'Нет связи с сервером Nova');
  }
  const type = res.headers.get('content-type') || '';
  const data = type.indexOf('json') >= 0 ? await res.json() : null;
  if (!res.ok) {
    throw new ApiError(res.status, (data && data.error) || 'error', (data && data.message) || 'Ошибка ' + res.status);
  }
  return data as T;
}

export const put = <T = unknown>(path: string, json?: unknown) => api<T>(path, { method: 'PUT', json: json === undefined ? {} : json });
export const post = <T = unknown>(path: string, json?: unknown) => api<T>(path, { method: 'POST', json: json === undefined ? {} : json });

export function errorText(e: unknown): string {
  return e instanceof ApiError ? e.message : 'Что-то пошло не так';
}
