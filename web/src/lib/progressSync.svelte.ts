// Синхронизация места просмотра между устройствами. Сервер в `/api/badges` сообщает, когда зритель
// последний раз сохранял место (`watchedAt`). Выросло, а это устройство само ничего не сохраняло, -
// смотрели на другом: `stamp` растёт, и «Продолжить», каталог и открытый плеер перечитывают место.

export const progressSync = $state({ stamp: 0 });

/** -1 - ещё не знаем (первый ответ значков только запоминаем). */
let known = -1;
let ownSave = false;

/** Плеер этого устройства сохранил место - следующий рост `watchedAt` наш, а не чужой. */
export function noteOwnSave() {
  ownSave = true;
}

/** Ответ значков: решить, смотрели ли на другом устройстве. */
export function noteWatchedAt(at: number | undefined) {
  if (typeof at !== 'number') return;
  if (known >= 0 && at > known && !ownSave) progressSync.stamp++;
  if (at > known) known = at;
  ownSave = false;
}
