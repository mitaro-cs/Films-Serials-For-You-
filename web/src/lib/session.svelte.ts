import { api } from './api';
import type { Me } from './types';

/** Сервер не ответил сам (компьютер хоста выключен, туннель показывает свою страницу). */
export const net = $state({ down: false });

export const session = $state<{ me: Me | null; loading: boolean }>({ me: null, loading: true });

export async function loadMe(): Promise<Me | null> {
  try {
    session.me = await api<Me>('/api/me');
  } catch {
    session.me = null;
  } finally {
    session.loading = false;
  }
  return session.me;
}
