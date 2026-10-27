// Снимки интерфейса на демо-данных e2e (для сверки дизайна): node scripts/look.mjs [папка]
// Нужен собранный сервер (FSFY_BIN или target/release). Страницы - компьютер 1440×900 и телефон 390×844.
import { chromium } from '@playwright/test';
import { mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import setup from '../e2e/setup.ts';

const out = process.argv[2] ?? 'look';
mkdirSync(out, { recursive: true });
const teardown = await setup();
const env = JSON.parse(readFileSync(new URL('../e2e/.env.json', import.meta.url), 'utf8'));
const state = new URL('../e2e/.state.json', import.meta.url).pathname.replace(/^\/(\w:)/, '$1');
const pages = [
  ['home', '/'],
  ['series', '/?tab=series'],
  ['title', `/title/${env.ids['Ла-Ла Ленд']}`],
  ['show', `/title/${env.ids['Тьма']}`],
  ['ext', '/ext/tt1196946'],
];
// DEV=1 - интерфейс из исходников (vite dev) поверх того же сервера: без пересборки Rust.
// Куки входа - по адресу 127.0.0.1 без порта, так что сессия окна подходит и для vite.
let base = env.baseURL;
let vite = null;
if (process.env.DEV) {
  const { spawn } = await import('node:child_process');
  vite = spawn('npx', ['vite', 'dev', '--port', '5197', '--strictPort', '--host', '127.0.0.1'], {
    env: { ...process.env, FSFY_URL: env.baseURL },
    shell: true,
    stdio: ['ignore', 'pipe', 'inherit'],
  });
  await new Promise((ok) => vite.stdout.on('data', (d) => String(d).includes('5197') && ok()));
  base = 'http://127.0.0.1:5197';
}
env.baseURL = base;
const browser = await chromium.launch();
try {
  for (const [device, viewport] of [['desk', { width: 1440, height: 900 }], ['phone', { width: 390, height: 844 }]]) {
    const ctx = await browser.newContext({ viewport, storageState: state, locale: 'ru-RU', deviceScaleFactor: device === 'phone' ? 2 : 1 });
    const page = await ctx.newPage();
    page.on('pageerror', (e) => console.log('ОШИБКА', e.message));
    for (const [name, path] of pages) {
      await page.goto(env.baseURL + path);
      await page.waitForTimeout(1500);
      await page.screenshot({ path: join(out, `${name}-${device}.png`) });
    }
    // Плеер: пауза, панель видна.
    const nach = await (await page.request.get(`${env.baseURL}/api/titles/${env.ids['Начало']}`)).json();
    await page.goto(`${env.baseURL}/watch/${nach.files[0].id}`);
    await page.waitForTimeout(2000);
    await page.evaluate(() => document.querySelector('video')?.pause());
    await page.mouse.move(viewport.width / 2, viewport.height / 2);
    await page.waitForTimeout(400);
    await page.screenshot({ path: join(out, `player-${device}.png`) });
    await ctx.close();
  }
  console.log('готово:', out, env.baseURL);
  if (process.env.KEEP) await new Promise(() => {});
} finally {
  await browser.close();
  if (vite) {
    const { execSync } = await import('node:child_process');
    if (process.platform === 'win32') execSync(`taskkill /pid ${vite.pid} /T /F`, { stdio: 'ignore' });
    else vite.kill();
  }
  await teardown();
}
