// Цвет фильма для страницы: преобладающий оттенок кадра, яркость - по средней яркости. Считается в
// браузере по самому кадру (та же картинка, что на экране, - без лишних запросов и миграций).

/** RGB-пиксели подряд -> `#rrggbb`: преобладающий оттенок, яркость - по средней яркости кадра. */
export function toneOf(rgb: Uint8ClampedArray | number[]): string {
  const BUCKETS = 12;
  const sum = Array.from({ length: BUCKETS }, () => [0, 0, 0, 0]);
  const grey = [0, 0, 0, 0];
  let luma = 0;
  let n = 0;
  for (let i = 0; i + 2 < rgb.length; i += 3) {
    const [r, g, b] = [rgb[i] / 255, rgb[i + 1] / 255, rgb[i + 2] / 255];
    const max = Math.max(r, g, b);
    const min = Math.min(r, g, b);
    luma += 0.2126 * r + 0.7152 * g + 0.0722 * b;
    n++;
    const s = max > 0 ? (max - min) / max : 0;
    const lit = max >= 0.18 && max <= 0.95 ? 1 : 0.15;
    if (s < 0.15) {
      grey[0] += r * lit;
      grey[1] += g * lit;
      grey[2] += b * lit;
      grey[3] += lit;
      continue;
    }
    const k = Math.min(BUCKETS - 1, Math.floor((hue(r, g, b, max, min) / 360) * BUCKETS));
    const w = s * lit;
    sum[k][0] += r * w;
    sum[k][1] += g * w;
    sum[k][2] += b * w;
    sum[k][3] += w;
  }
  const best = sum.reduce((a, b) => (b[3] > a[3] ? b : a), [0, 0, 0, 0]);
  let r = 0.2;
  let g = 0.2;
  let b = 0.22;
  let colorful = false;
  if (best[3] > grey[3] * 0.08 && best[3] > 0) {
    [r, g, b] = [best[0] / best[3], best[1] / best[3], best[2] / best[3]];
    colorful = true;
  } else if (grey[3] > 0) {
    [r, g, b] = [grey[0] / grey[3], grey[1] / grey[3], grey[2] / grey[3]];
  }
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const h = hue(r, g, b, max, min);
  const l0 = (max + min) / 2;
  const s0 = max === min ? 0 : (max - min) / (1 - Math.abs(2 * l0 - 1));
  const s = colorful ? Math.min(0.6, Math.max(0.28, s0)) : Math.min(0.08, s0);
  // Светлый кадр - светлее фон, тёмный - темнее: от 0,2 до 0,42.
  const l = Math.min(0.42, Math.max(0.2, n ? (luma / n) * 0.9 + 0.1 : 0.3));
  return hex(...hsl(h, s, l));
}

function hue(r: number, g: number, b: number, max: number, min: number): number {
  const d = max - min;
  if (d <= 0) return 0;
  const h = max === r ? (((g - b) / d) % 6 + 6) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return h * 60;
}

function hsl(h: number, s: number, l: number): [number, number, number] {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = l - c / 2;
  const [r, g, b] = h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x] : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
  return [r + m, g + m, b + m];
}

const hex = (r: number, g: number, b: number) =>
  '#' + [r, g, b].map((v) => Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, '0')).join('');

const cache = new Map<string, string>();

/** Цвет по адресу кадра (тот же источник, что на экране); не вышло - null. Помним в памяти и localStorage. */
export async function toneOfUrl(url: string): Promise<string | null> {
  const known = cache.get(url);
  if (known) return known;
  try {
    const saved = localStorage.getItem('tone:' + url);
    if (saved) {
      cache.set(url, saved);
      return saved;
    }
  } catch {
    /* приватный режим */
  }
  try {
    const img = new Image();
    img.decoding = 'async';
    img.src = url;
    await img.decode();
    const canvas = document.createElement('canvas');
    canvas.width = 32;
    canvas.height = 18;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    if (!ctx) return null;
    ctx.drawImage(img, 0, 0, 32, 18);
    const data = ctx.getImageData(0, 0, 32, 18).data;
    const rgb: number[] = [];
    for (let i = 0; i < data.length; i += 4) rgb.push(data[i], data[i + 1], data[i + 2]);
    const tone = toneOf(rgb);
    cache.set(url, tone);
    try {
      if (localStorage.length < 400) localStorage.setItem('tone:' + url, tone);
    } catch {
      /* места нет */
    }
    return tone;
  } catch {
    return null;
  }
}
