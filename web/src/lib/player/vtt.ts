export interface Cue {
  start: number;
  end: number;
  text: string;
}

function time(s: string): number {
  const p = s.trim().replace(',', '.').split(':').map(Number);
  return p.length === 3 ? p[0] * 3600 + p[1] * 60 + p[2] : p[0] * 60 + p[1];
}

/** WebVTT → реплики. Теги и стили ASS убираются: показываем свой текст. */
export function parseVtt(src: string): Cue[] {
  const cues: Cue[] = [];
  for (const block of src.replace(/\r/g, '').split(/\n{2,}/)) {
    const lines = block.split('\n');
    const i = lines.findIndex((l) => l.includes('-->'));
    if (i < 0) continue;
    const [a, b] = lines[i].split('-->');
    const text = lines
      .slice(i + 1)
      .join('\n')
      .replace(/<[^>]+>/g, '')
      .replace(/\{\\[^}]*\}/g, '')
      .trim();
    if (text) cues.push({ start: time(a), end: time(b.trim().split(/\s+/)[0]), text });
  }
  return cues.sort((x, y) => x.start - y.start);
}

/** Реплика на момент `t` (двоичный поиск по началу). */
export function cueAt(cues: Cue[], t: number): string {
  let lo = 0;
  let hi = cues.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (cues[mid].start <= t) {
      found = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  const out: string[] = [];
  for (let k = found; k >= 0 && k > found - 4; k--) if (cues[k].end > t) out.unshift(cues[k].text);
  return out.join('\n');
}
