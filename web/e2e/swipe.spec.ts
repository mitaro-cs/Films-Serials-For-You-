import { expect, test } from './util-test';
import { url } from './util';

test('главная переключает кадр свайпом на телефоне', async ({page}) => {
  await page.goto(url('/'));
  const hero=page.getByRole('region',{name:'Рекомендуем'});
  const tabs=hero.getByRole('tab');
  await expect(tabs.first()).toBeVisible();
  expect(await tabs.count()).toBeGreaterThan(1);
  const previous=await hero.getByRole('tab',{selected:true}).getAttribute('aria-label');
  await hero.dispatchEvent('pointerdown',{pointerType:'touch',clientX:320,clientY:200});
  await hero.dispatchEvent('pointerup',{pointerType:'touch',clientX:100,clientY:205});
  await expect(hero.getByRole('tab',{selected:true})).not.toHaveAttribute('aria-label',previous!);
});
