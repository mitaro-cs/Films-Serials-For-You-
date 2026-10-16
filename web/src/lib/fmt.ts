export function runtime(min: number | null | undefined): string {
  if (!min) return '';
  const h = Math.floor(min / 60);
  const m = min % 60;
  return h ? `${h} ч ${m ? `${m} мин` : ''}`.trim() : `${m} мин`;
}

export function clock(sec: number): string {
  if (!isFinite(sec) || sec < 0) sec = 0;
  const s = Math.floor(sec % 60);
  const m = Math.floor((sec / 60) % 60);
  const h = Math.floor(sec / 3600);
  const pad = (n: number) => String(n).padStart(2, '0');
  return h ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

export function size(bytes: number): string {
  const units = ['Б', 'КБ', 'МБ', 'ГБ', 'ТБ'];
  let i = 0;
  let v = bytes;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 && i > 0 ? 1 : 0).replace('.', ',')} ${units[i]}`;
}

/** 1 сериал, 2 сериала, 5 сериалов. */
export function plural(n: number, one: string, few: string, many: string): string {
  const a = Math.abs(n) % 100;
  const b = a % 10;
  if (a > 10 && a < 20) return many;
  if (b > 1 && b < 5) return few;
  if (b === 1) return one;
  return many;
}

export function count(n: number, one: string, few: string, many: string): string {
  return `${n} ${plural(n, one, few, many)}`;
}

const MONTHS = ['января', 'февраля', 'марта', 'апреля', 'мая', 'июня', 'июля', 'августа', 'сентября', 'октября', 'ноября', 'декабря'];

export function date(iso: string | null | undefined): string {
  if (!iso) return '';
  const [y, m, d] = iso.split('-').map(Number);
  if (!m) return String(y);
  if (!d) return `${MONTHS[m - 1].replace(/я$/, 'ь').replace(/а$/, '')} ${y}`;
  return `${d} ${MONTHS[m - 1]} ${y}`;
}

export function ago(ms: number | null | undefined): string {
  if (!ms) return 'никогда';
  const s = (Date.now() - ms) / 1000;
  if (s < 90) return 'только что';
  if (s < 3600) return `${Math.round(s / 60)} мин назад`;
  if (s < 86400) return `${Math.round(s / 3600)} ч назад`;
  const d = Math.round(s / 86400);
  return d < 30 ? `${count(d, 'день', 'дня', 'дней')} назад` : new Date(ms).toLocaleDateString('ru-RU');
}

export function votes(n: number | null | undefined): string {
  if (!n) return '';
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace('.', ',')} млн`;
  if (n >= 1000) return `${Math.round(n / 1000)} тыс.`;
  return String(n);
}

export function kindLabel(kind: string): string {
  return kind === 'series' ? 'Сериал' : 'Фильм';
}
