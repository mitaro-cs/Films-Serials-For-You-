/** Familiar encoding tiers; selection still uses the real dimensions from the manifest. */
export function qualityTier(height: number, width?: number) {
  const nominal = width && width / height > 1.9 ? width * 9 / 16 : height;
  return [144,240,360,480,720,1080,1440,2160,4320].reduce((best,v) =>
    Math.abs(v-nominal)<Math.abs(best-nominal)?v:best,144);
}
