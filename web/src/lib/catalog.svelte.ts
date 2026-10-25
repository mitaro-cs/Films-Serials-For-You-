import { api } from './api';
import type { Card, Library } from './types';

// Весь каталог одним запросом и в памяти вкладки: Главная, Фильмы и Сериалы переключаются
// мгновенно, а возврат на главную показывает прежнее сразу и тихо обновляет (не чаще раза в 15 с).

export const catalog = $state<{ items: Card[] | null; recent: number[]; failed: boolean }>({
  items: null,
  recent: [],
  failed: false,
});

let at = 0;
let pending: Promise<void> | null = null;

export function loadCatalog(force = false): Promise<void> {
  if (pending) return pending;
  if (!force && catalog.items && Date.now() - at < 15_000) return Promise.resolve();
  pending = api<Library>('/api/library')
    .then(
      (r) => {
        catalog.items = r.items;
        catalog.recent = r.recent;
        catalog.failed = false;
        at = Date.now();
      },
      () => {
        if (!catalog.items) catalog.failed = true;
      },
    )
    .finally(() => (pending = null));
  return pending;
}

/** После выхода - чужой каталог не показываем. */
export function forgetCatalog() {
  catalog.items = null;
  catalog.recent = [];
  at = 0;
}
