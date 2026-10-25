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
