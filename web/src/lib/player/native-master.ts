import { qualityTier } from './quality-label';
/** Safari plays HLS natively; expose the resolutions advertised by its master playlist. */
export function nativeLevels(text: string) {
  const variants: { i: number; height: number; label: number }[] = [];
  for (const line of text.split(/\r?\n/)) {
    if (!line.startsWith('#EXT-X-STREAM-INF:')) continue;
    const resolution = /RESOLUTION=(\d+)x(\d+)/.exec(line);
    const height = Number(resolution?.[2]);
    variants.push({i: variants.length, height, label: qualityTier(height,Number(resolution?.[1]))});
  }
  return variants.filter(v => v.height > 0)
    .filter((v, i, all) => all.findIndex(x => x.label === v.label) === i)
    .sort((a,b) => b.height-a.height);
}
