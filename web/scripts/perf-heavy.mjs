// Что весит больше всего на странице: самые тяжёлые ответы (картинки, код) с размерами.
// Запуск из web/: node scripts/perf-heavy.mjs <папка данных> <fsfy.exe> <страница> [phone]
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';

const [data, bin, path, phone] = process.argv.slice(2);
const child = spawn(bin, ['serve', '--data', data, '--port', '19810', '--window-port', '19811'], { env: { ...process.env, RUST_LOG: 'warn' }, stdio: ['pipe', 'pipe', 'inherit'] });
process.on('exit', () => child.kill());
const entry = await new Promise((ok) => { let out = ''; child.stdout.on('data', (d) => { out += d; const m = out.match(/Вход хоста:\s+(\S+)/); if (m) ok(m[1]); }); });
const browser = await chromium.launch();
const ctx = await browser.newContext(phone ? { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 3 } : { viewport: { width: 1440, height: 900 } });
const page = await ctx.newPage();
await page.goto(entry);
const list = [];
page.on('requestfinished', async (r) => {
  const s = await r.sizes().catch(() => null);
  const res = await r.response();
  list.push({ url: new URL(r.url()).pathname.slice(0, 70), тип: res?.headers()['content-type']?.split(';')[0], КБ: Math.round((s?.responseBodySize ?? 0) / 1024) });
});
await page.goto('http://127.0.0.1:19811' + path, { waitUntil: 'load', timeout: 90_000 }).catch((e) => console.log('goto', e.message));
await page.waitForTimeout(12_000);
const imgs = await page.evaluate(() => [...document.images].filter((i) => i.complete && i.naturalWidth).map((i) => ({ src: new URL(i.currentSrc).pathname.slice(0, 50), натур: `${i.naturalWidth}x${i.naturalHeight}`, показ: `${Math.round(i.getBoundingClientRect().width)}x${Math.round(i.getBoundingClientRect().height)}`, loading: i.loading })));
const byType = {};
for (const r of list) byType[r.тип] = (byType[r.тип] ?? 0) + r.КБ;
console.log('По типам, КБ:', byType);
console.table(list.sort((a, b) => b.КБ - a.КБ).slice(0, 12));
console.log('Картинки: размер файла против показа (первые 15 самых больших):');
console.table(imgs.sort((a, b) => parseInt(b.натур) - parseInt(a.натур)).slice(0, 15));
await browser.close();
child.kill();
process.exit(0);
