import { describe, it, expect } from 'vitest';
import { nativeLevels } from './native-master';
describe('native HLS qualities', () => {
  it('deduplicates resolutions and preserves the master variant index', () => {
    const levels = nativeLevels('#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,URI="audio.m3u8"\n#EXT-X-STREAM-INF:RESOLUTION=1280x720,AUDIO="a"\n720.m3u8\n#EXT-X-STREAM-INF:RESOLUTION=640x360\n360.m3u8\n#EXT-X-STREAM-INF:RESOLUTION=1280x720\n720b.m3u8');
    expect(levels).toEqual([{i:0,height:720,label:720},{i:1,height:360,label:360}]);
  });
  it('does not invent qualities in a media playlist', () => {
    expect(nativeLevels('#EXTM3U\n#EXTINF:6,\nseg.ts')).toEqual([]);
  });
});
