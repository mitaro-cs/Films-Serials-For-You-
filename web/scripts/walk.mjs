// Обход экранов: снимки каждой страницы на телефоне (и экран ниже) - найти, что съехало или сломано.
// Запуск из web/: node scripts/walk.mjs <папка данных> <fsfy.exe> <папка для снимков> [ширина высота]
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';

const [data, bin, out, w = '390', h = '844'] = process.argv.slice(2);
mkdirSync(out, { recursive: true });
const child = spawn(bin, ['serve', '--data', data, '--port', '19820', '--window-port', '19821'], { env: { ...process.env, RUST_LOG: 'warn' }, stdio: ['pipe', 'pipe', 'inherit'] });
process.on('exit', () => child.kill());
const entry = await new Promise((ok) => { let s = ''; child.stdout.on('data', (d) => { s += d; const m = s.match(/Вход хоста:\s+(\S+)/); if (m) ok(m[1]); }); });
const db = new DatabaseSync(`${data}/fsfy.db`, { readOnly: true });
const title = db.prepare('SELECT id FROM titles LIMIT 1').get()?.id;
const file = db.prepare('SELECT id FROM files LIMIT 1').get()?.id;
const ext = db.prepare('SELECT ext_id FROM ext_charts ORDER BY pos LIMIT 1').get()?.ext_id;
const extSeries = db.prepare("SELECT id FROM ext_titles WHERE kind = 'series' AND detailed > 0 LIMIT 1").get()?.id;
const person = db.prepare('SELECT id FROM profiles ORDER BY id DESC LIMIT 1').get()?.id;
const list = db.prepare('SELECT id FROM lists LIMIT 1').get()?.id;
db.close();
const PAGES = [
  ['home', '/'], ['movies', '/movies'], ['series', '/series'], ['anime', '/?tab=anime'], ['discover', '/discover'],
  ['title', `/title/${title}`], ['ext', `/ext/${ext}`], ['ext-series', `/ext/${extSeries}`], ['search', '/search?q=гарри'],
  ['people', '/people'], ['person', `/people/${person}`], ['chats', '/chats'], ['lists', '/lists'], ['list', `/lists/${list}`],
  ['profile', '/profile'], ['admin', '/admin'], ['watch', `/watch/${file}`],
];
const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: +w, height: +h }, isMobile: +w < 700, hasTouch: +w < 700, deviceScaleFactor: 1 });
const page = await ctx.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(`${page.url()}: ${e.message}`));
page.on('console', (m) => { if (m.type() === 'error') errors.push(`${page.url()}: ${m.text()}`); });
await page.goto(entry);
for (const [name, path] of PAGES) {
  await page.goto('http://127.0.0.1:19821' + path, { waitUntil: 'load' }).catch(() => {});
  await page.waitForTimeout(3500);
  await page.screenshot({ path: `${out}/${name}-1.png` });
  await page.mouse.wheel(0, +h * 0.9);
  await page.waitForTimeout(1200);
  await page.screenshot({ path: `${out}/${name}-2.png` });
  // Шире экрана - виновник.
  const wide = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1 ? document.documentElement.scrollWidth : 0);
  if (wide) errors.push(`${path}: страница шире экрана (${wide}px)`);
}
console.log(errors.length ? errors.join('\n') : 'ошибок JS нет');
await browser.close();
child.kill();
process.exit(0);
