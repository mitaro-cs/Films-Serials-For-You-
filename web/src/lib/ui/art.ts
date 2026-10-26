/** Цвет заглушки постера по названию - у каждого свой, но стабильный. */
export function hue(s: string): number {
  let h = 0;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return h % 360;
}

export function placeholder(title: string): string {
  const shade = 4 + hue(title) % 7;
  return `linear-gradient(160deg, rgb(${shade} ${shade} ${shade}), #000)`;
}

const ART = '/api/discover/art?';

/** Картинка IMDb нужной ширины: сервер уменьшит её один раз и запомнит. Свои (/img/…) - как есть. */
export function artW(url: string | null | undefined, w: number): string | undefined {
  if (!url) return undefined;
  return url.startsWith(ART) && !url.includes('&w=') ? `${url}&w=${w}` : url;
}

/** Набор ширин для большого кадра: телефон возьмёт 640-1280 px, а не 2048. */
export function artSet(url: string | null | undefined): string | undefined {
  if (!url?.startsWith(ART) || url.includes('&w=')) return undefined;
  return `${artW(url, 640)} 640w, ${artW(url, 1280)} 1280w, ${url} 2048w`;
}

/** Большой кадр героя: свой фильм - 640 px на телефоне и 1920 на компьютере, кадр IMDb - как `artSet`. */
export function heroSet(big: string | null | undefined, small: string | null | undefined): string | undefined {
  if (!big) return undefined;
  if (big.startsWith(ART)) return artSet(big);
  return small && small !== big ? `${small} 640w, ${big} 1920w` : undefined;
}
