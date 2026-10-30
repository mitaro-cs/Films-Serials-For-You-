/** Only dates known to be in the future are treated as unreleased. */
export function upcomingYear(value: string | null | undefined, now = new Date()): number | null {
  const match = /^(\d{4})(?:-(\d{2})(?:-(\d{2}))?)?$/.exec(value ?? '');
  if (!match) return null;
  const year = Number(match[1]), month = match[2] ? Number(match[2]) : 1, day = match[3] ? Number(match[3]) : 1;
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  const current = now.getFullYear() * 10000 + (now.getMonth() + 1) * 100 + now.getDate();
  if (!match[2]) return year > now.getFullYear() ? year : null;
  if (!match[3]) return year * 100 + month > now.getFullYear() * 100 + now.getMonth() + 1 ? year : null;
  return year * 10000 + month * 100 + day > current ? year : null;
}
