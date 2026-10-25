import { expect, it } from 'vitest';
import { trailerHeight } from './trailer-quality';
it('selects HD using actual cropped resolution and rejects SD-only previews', () => {
  expect(trailerHeight('RESOLUTION=640x344\nRESOLUTION=1280x688\nRESOLUTION=1920x1032')).toBe(1032);
  expect(trailerHeight('RESOLUTION=1920x800\nRESOLUTION=3840x1600')).toBe(800);
  expect(trailerHeight('RESOLUTION=640x360')).toBeNull();
});
