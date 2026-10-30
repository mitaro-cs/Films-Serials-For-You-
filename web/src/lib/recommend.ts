import type { ExtCard } from './types';

// Рекомендации на главной: что из подборок IMDb подойдёт именно этому человеку. Вкус - вес у каждого
// жанра (названные при знакомстве, любимые, начатые, «Хочу» и просмотренные), у фильма - сумма весов его
// жанров, плюс оценка IMDb и свежесть. Не больше нескольких подряд одного жанра - чтобы лента не
// превращалась в десять боевиков.

export type Taste = Map<string, number>;

/** Добавить вес жанрам (русские названия). */
export function like(taste: Taste, genres: string[], weight: number) {
  for (const g of genres) taste.set(g, (taste.get(g) ?? 0) + weight);
}

export function score(f: Pick<ExtCard, 'genres' | 'rating' | 'year'>, taste: Taste, genreRu: (g: string) => string, year = new Date().getFullYear()): number {
  const matched = f.genres.map(genreRu).filter((g) => taste.has(g));
  if (!matched.length) return 0;
  const affinity = matched.reduce((s, g) => s + (taste.get(g) ?? 0), 0);
  // Оценка: выше 7 - плюс, ниже 6 - минус; свежее (до 8 лет) - чуть выше.
  const quality = ((f.rating ?? 6.5) - 6.5) * 0.9;
  const fresh = f.year && year - f.year <= 8 ? 0.8 : 0;
  return affinity + quality + fresh;
}

export function recommend(
  items: ExtCard[],
  taste: Taste,
  skip: (f: ExtCard) => boolean,
  genreRu: (g: string) => string,
  n = 20,
  perGenre = 6,
): ExtCard[] {
  const seen = new Set<string>();
  const ranked = items
    .filter((f) => {
      if (seen.has(f.id) || skip(f)) return false;
      seen.add(f.id);
      return true;
    })
    .map((f) => ({ f, s: score(f, taste, genreRu) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s);
  const per = new Map<string, number>();
  const out: ExtCard[] = [];
  for (const { f } of ranked) {
    const lead = f.genres.map(genreRu).find((g) => taste.has(g)) ?? '';
    if ((per.get(lead) ?? 0) >= perGenre) continue;
    per.set(lead, (per.get(lead) ?? 0) + 1);
    out.push(f);
    if (out.length >= n) break;
  }
  return out;
}
