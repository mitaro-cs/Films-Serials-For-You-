// Короткие уведомления внизу экрана («Отправлено», «Добавлено 3 файла»). Сами исчезают.
export const toasts = $state<{ id: number; text: string }[]>([]);
let next = 1;

export function toast(text: string, ms = 3500) {
  const id = next++;
  toasts.push({ id, text });
  setTimeout(() => {
    const i = toasts.findIndex((t) => t.id === id);
    if (i >= 0) toasts.splice(i, 1);
  }, ms);
}
