import { session } from './session.svelte';

/** Не показываем английский сюжет до готовности русского. Оригинальное название остаётся отдельно. */
export function localized(text: string | null | undefined, fallback = ''): string {
  if (!text) return fallback;
  if (session.me?.site.metadataLanguage === 'source' || /[а-яё]/i.test(text) || !/[a-z]/i.test(text)) return text;
  return fallback;
}
