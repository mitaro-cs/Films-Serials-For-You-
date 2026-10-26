// Чёрные полосы в видео (фильм 2.39:1 внутри 16:9 или ещё и в рамке 4:3): какая доля высоты сверху и
// снизу пустая. По одному кадру не решить (тёмная сцена тоже «чёрная»), поэтому берём наименьшее по
// нескольким кадрам.

/** Строка пустая: почти чёрная по всей ширине. */
function darkRow(data: Uint8ClampedArray, width: number, y: number): boolean {
  let sum = 0;
  for (let x = 0; x < width; x++) {
    const i = (y * width + x) * 4;
    const l = (data[i] * 2 + data[i + 1] * 5 + data[i + 2]) >> 3;
    if (l > 40) return false;
    sum += l;
  }
  return sum / width < 16;
}

/** Доля пустой высоты с каждой стороны (меньшая из верха и низа) у пикселей кадра. */
export function barsOf(data: Uint8ClampedArray, width: number, height: number): number {
  let top = 0;
  while (top < height / 2 && darkRow(data, width, top)) top++;
  let bottom = 0;
  while (bottom < height / 2 && darkRow(data, width, height - 1 - bottom)) bottom++;
  return Math.min(top, bottom) / height;
}

const W = 64;
const H = 36;

/** Доля полосы в текущем кадре видео; null - кадр не прочитать (чужой адрес, ещё нет кадра). */
export function measure(video: HTMLVideoElement): number | null {
  if (!video.videoWidth || video.readyState < 2) return null;
  try {
    const canvas = document.createElement('canvas');
    canvas.width = W;
    canvas.height = H;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    if (!ctx) return null;
    ctx.drawImage(video, 0, 0, W, H);
    return barsOf(ctx.getImageData(0, 0, W, H).data, W, H);
  } catch {
    return null;
  }
}

/** Во сколько раз увеличить видео, чтобы полосы ушли за край (не больше 1,6). */
export function zoomFor(bar: number): number {
  if (bar < 0.03) return 1;
  return Math.min(1.6, 1 / (1 - 2 * bar));
}
