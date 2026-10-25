// Скорость до хоста у этого зрителя: плеер качает несжимаемый кусок с `/api/speedtest` и решает,
// с какого качества начать. Замер помнится на устройстве 10 минут - следующая серия стартует сразу.

const KEY = 'nova-speed';
const FRESH_MS = 10 * 60_000;
const BYTES = 1_500_000;
const LIMIT_MS = 2_500;

type Saved = { mbit: number; at: number };

function saved(): number | null {
  try {
    const s = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Saved | null;
    return s && Date.now() - s.at < FRESH_MS && Number.isFinite(s.mbit) ? s.mbit : null;
  } catch {
    return null;
  }
}

function remember(mbit: number) {
  try {
    localStorage.setItem(KEY, JSON.stringify({ mbit, at: Date.now() } satisfies Saved));
  } catch {
    /* приватный режим */
  }
}

/** Забыть замер: после остановок видео скорость явно хуже записанной. */
export function forgetSpeed() {
  try {
    localStorage.removeItem(KEY);
  } catch {
    /* приватный режим */
  }
}

/** Мбит/с до хоста или null (не вышло). Считаем от первого байта: ожидание ответа - не скорость. */
export async function measureSpeed(): Promise<number | null> {
  const known = saved();
  if (known !== null) return known;
  const stop = new AbortController();
  const timer = setTimeout(() => stop.abort(), LIMIT_MS);
  let got = 0;
  let first = 0;
  try {
    const r = await fetch(`/api/speedtest?bytes=${BYTES}`, { credentials: 'same-origin', cache: 'no-store', signal: stop.signal });
    if (!r.ok || !r.body) return null;
    const reader = r.body.getReader();
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (!first) first = performance.now();
      else got += value.byteLength;
    }
  } catch {
    // Не успели за 2,5 секунды - считаем по тому, что пришло: это тоже ответ (медленно).
  } finally {
    clearTimeout(timer);
  }
  const ms = first ? performance.now() - first : 0;
  // Мало байт за короткое время - не замер; мало за секунду и дольше - это и есть медленный интернет.
  if (ms <= 0 || (got < 64_000 && ms < 1_000)) return null;
  const mbit = Math.round(((got * 8) / (ms / 1000) / 1e6) * 10) / 10;
  remember(mbit);
  return mbit;
}

/** С чего начать: «Оригинал», если скорости с запасом в полтора раза; иначе - автокачество. */
export function startQuality(mbit: number | null, bitrate: number | null): 'orig' | 'auto' {
  if (mbit === null || bitrate === null || bitrate <= 0) return 'orig';
  return mbit >= bitrate * 1.5 ? 'orig' : 'auto';
}

/** Начальная оценка для автокачества (бит/с): с запасом, чтобы первый кусок не застрял. */
export function abrEstimate(mbit: number | null): number | undefined {
  return mbit === null ? undefined : Math.max(300_000, mbit * 1e6 * 0.75);
}
