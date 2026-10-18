import type { ExtCard } from '$lib/types';

// Карточка фильма из ленты открывается поверх любой страницы — одна на приложение.
export const extSheet = $state<{ id: string | null; seed: ExtCard | null; version: number }>({
  id: null,
  seed: null,
  version: 0,
});

export function openExt(card: ExtCard) {
  extSheet.seed = card;
  extSheet.id = card.id;
}

/** Что-то поменялось (отметка, список) — страницы, которые показывают карточки, обновятся. */
export function extChanged() {
  extSheet.version++;
}
