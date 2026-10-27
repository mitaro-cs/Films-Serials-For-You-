// Что браузер умеет показать «как в файле». HEVC спрашиваем полной строкой кодека: на голое
// `codecs="hvc1"` Chrome и Edge на Windows отвечают «нет» даже с аппаратным HEVC - из-за этого
// 4K-фильмы (почти всегда HEVC, часто 10 бит) никогда не шли «Оригиналом» и упирались в 1080p.

type CanPlay = (type: string) => string;

/** HEVC Main (8 бит) и Main10 (10 бит, 4K HDR) - по отдельности: бывают устройства только с Main. */
export function hevcSupport(canPlay: CanPlay | null): { main: boolean; main10: boolean } {
  if (!canPlay) return { main: false, main10: false };
  const any = (codecs: string[]) => codecs.some((c) => canPlay(`video/mp4; codecs="${c}"`) !== '');
  // Уровень 5.1 (L153) - 4K; кто умеет его, умеет и меньше.
  return {
    main: any(['hvc1.1.6.L153.B0', 'hev1.1.6.L153.B0', 'hvc1.1.6.L120.90', 'hvc1']),
    main10: any(['hvc1.2.4.L153.B0', 'hev1.2.4.L153.B0', 'hvc1.2.4.L120.90']),
  };
}

/** Подпись ступени качества: 2160 - «4K», остальное - «1080p». */
export function heightLabel(h: number): string {
  return h >= 2000 ? '4K' : `${h}p`;
}
