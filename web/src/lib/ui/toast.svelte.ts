// Короткие уведомления внизу экрана («Отправлено», «Добавлено 3 файла»). Сами исчезают.
// С действием - кнопка рядом («Убрано · Вернуть»): нажали - уведомление пропадает сразу.
export interface ToastAction {
  label: string;
  run: () => void;
}

export const toasts = $state<{ id: number; text: string; action?: ToastAction }[]>([]);
let next = 1;

export function dismiss(id: number) {
  const i = toasts.findIndex((t) => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

export function toast(text: string, ms = 3500, action?: ToastAction) {
  const id = next++;
  toasts.push({ id, text, action });
  setTimeout(() => dismiss(id), action ? Math.max(ms, 6000) : ms);
}
