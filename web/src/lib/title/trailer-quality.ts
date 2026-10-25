import { qualityTier } from '$lib/player/quality-label';

/** Select actual manifest resolution; never stretch an SD variant across the banner. */
export function trailerHeight(manifest: string): number | null {
  const variants = [...manifest.matchAll(/RESOLUTION=(\d+)x(\d+)/g)]
    .map((m) => ({ height: Number(m[2]), tier: qualityTier(Number(m[2]), Number(m[1])) }))
    .filter((v) => v.tier >= 720 && v.tier <= 1080)
    .sort((a, b) => b.tier - a.tier || b.height - a.height);
  return variants[0]?.height ?? null;
}
