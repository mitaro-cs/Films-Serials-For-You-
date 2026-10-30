import { expect, test } from './util-test';
import { env, expectClean, url } from './util';
import { execFileSync } from 'node:child_process';
import { canDecodeWebm } from './media-capability';

test('управление появляется без паузы, клавиши показывают результат', async ({ page }, info) => {
  // Headless Chromium lacks H.264 on this Windows runner: serve a real VP8 test clip.
  const clip = execFileSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-f', 'lavfi', '-i',
    'testsrc2=size=320x180:rate=12:duration=30', '-c:v', 'libvpx', '-deadline', 'realtime', '-f', 'webm', 'pipe:1'], { maxBuffer: 8 * 1024 * 1024 });
  test.skip(!await canDecodeWebm(page,clip),'VP8 WebM не декодируется этой тестовой средой');
  await page.route('**/api/files/*/raw', (route) => route.fulfill({ contentType: 'video/webm', body: clip }));
  await page.goto(url(`/title/${env().ids['Ла-Ла Ленд']}`));
  await page.getByRole('link', { name: /Продолжить|Смотреть/ }).first().click();
  const player = page.getByRole('application', { name: 'Видеоплеер' });
  await expect(player).toBeVisible();
  // После перехода курсор может остаться над нижними кнопками: там управление должно сохраняться.
  await page.mouse.move(100, 120);
  await page.waitForFunction(() => (document.querySelector('video')?.readyState ?? 0) >= 2);
  // Короткий тестовый ролик зациклен, чтобы проверить скрытие во время воспроизведения.
  await page.locator('video').evaluate(async (v: HTMLVideoElement) => { v.loop = true; v.muted = true; await v.play(); });
  await expect(player).toHaveClass(/idle/, { timeout: 8000 });
  await page.mouse.move(100, 120);
  await expect(player).not.toHaveClass(/idle/);
  await page.keyboard.press('ArrowDown');
  await expect(player.getByRole('status')).toContainText('Громкость 90%');
  await page.keyboard.press('ArrowRight');
  await expect(player.getByRole('status')).toContainText('Вперёд на 10 секунд');
  await page.keyboard.press('KeyM');
  await expect(player.getByRole('status')).toContainText('Звук выключен');
  await page.getByRole('group', { name: 'Управление просмотром' }).hover();
  await page.waitForTimeout(3200);
  await expect(player).not.toHaveClass(/idle/);
  await expect(player).toHaveClass(/idle/, {timeout:2500});
  expect(await player.locator('.bottom').evaluate(el => getComputedStyle(el).userSelect)).toBe('none');
  await page.mouse.move(100, 120);
  await expect(player).not.toHaveClass(/idle/);
  await expectClean(page, 'плеер');
  await page.screenshot({ path: `test-results/player-${info.project.name}.png` });
});

test('правила и конфиденциальность доступны без входа', async ({ browser }) => {
  const context = await browser.newContext({ storageState: { cookies: [], origins: [] } });
  const page = await context.newPage();
  for (const doc of ['terms', 'privacy']) {
    await page.goto(url(`/legal/${doc}`));
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
    await expect(page).toHaveURL(new RegExp(`/legal/${doc}$`));
    await expectClean(page, doc);
  }
  await context.close();
});
