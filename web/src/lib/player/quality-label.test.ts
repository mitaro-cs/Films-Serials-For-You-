import {expect,it} from 'vitest';
import {qualityTier} from './quality-label';
it('labels cropped source resolutions with conventional tiers',()=>{
 expect([136,232,344,456,688,1032].map(v=>qualityTier(v))).toEqual([144,240,360,480,720,1080]);
 expect(qualityTier(800,1920)).toBe(1080);
 expect(qualityTier(1716,3840)).toBe(2160);
});
