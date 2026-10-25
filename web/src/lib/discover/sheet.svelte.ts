import { goto } from '$app/navigation';
import type { ExtCard } from '$lib/types';

// Фильм из ленты IMDb открывается своей страницей `/ext/<id>`. Карточку, по которой нажали,
// помним: страница сразу показывает постер и название, остальное приходит следом.
export const extSheet = $state<{ seed: ExtCard | null; version: number }>({
  seed: null,
  version: 0,
});

export function remember(card: ExtCard) {
  extSheet.seed = card;
}

export function openExt(card: ExtCard) {
  remember(card);
  goto(`/ext/${card.id}`);
}

/** Что-то поменялось (отметка, список) - страницы, которые показывают карточки, обновятся. */
export function extChanged() {
  extSheet.version++;
}
