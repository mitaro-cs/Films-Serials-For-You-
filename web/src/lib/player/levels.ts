import type { Trickplay } from '$lib/types';

/** Ступени качества HLS — так же, как на сервере (`hls::levels`). */
export function levels(sourceHeight: number | null): number[] {
  const h = sourceHeight ?? 1080;
  if (h < 360) return [h - (h % 2)];
  return [1080, 720, 480, 360].filter((l) => l <= Math.max(h, 360));
}

/** Кадр для полосы перемотки: лист и смещение внутри него. */
export function trickFrame(t: Trickplay, time: number) {
  const i = Math.max(0, Math.min(t.count - 1, Math.floor(time / t.interval)));
  const per = t.cols * t.rows;
  const sheet = Math.floor(i / per);
  const k = i % per;
  return { sheet, x: (k % t.cols) * t.width, y: Math.floor(k / t.cols) * t.height };
}
