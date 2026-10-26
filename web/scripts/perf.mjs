// Замер скорости Nova на копии данных владельца: каждая страница с холодного старта (как первый
// заход) - на «телефоне через туннель» (4G, процессор в 4 раза медленнее) и на компьютере.
// Запуск из web/: node <этот файл> <папка данных> <fsfy.exe>
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { DatabaseSync } from 'node:sqlite';

const [data, bin] = process.argv.slice(2);
const port = 19800, win = 19801;
const child = spawn(bin, ['serve', '--data', data, '--port', String(port), '--window-port', String(win)], {
  env: { ...process.env, RUST_LOG: 'warn' }, stdio: ['pipe', 'pipe', 'inherit'],
});
process.on('exit', () => child.kill());
const entry = await new Promise((ok) => {
  let out = '';
  child.stdout.on('data', (d) => { out += d; const m = out.match(/Вход хоста:\s+(\S+)/); if (m) ok(m[1]); });
});
const base = `http://127.0.0.1:${win}`;

const db = new DatabaseSync(`${data}/fsfy.db`, { readOnly: true });
const title = db.prepare('SELECT id FROM titles LIMIT 1').get()?.id;
const ext = db.prepare("SELECT ext_id FROM ext_charts ORDER BY pos LIMIT 1").get()?.ext_id;
db.close();
const PAGES = ['/', '/movies', '/series', '/?tab=anime', '/discover', `/title/${title}`, `/ext/${ext}`, '/search?q=гарри', '/people', '/chats', '/lists', '/profile', '/admin'];

const browser = await chromium.launch();
const login = await browser.newContext();
await (await login.newPage()).goto(entry);
const state = await login.storageState();
await login.close();

const PROFILES = {
  'телефон 4G': { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, net: { latency: 80, downloadThroughput: 1_250_000, uploadThroughput: 375_000 }, cpu: 4 },
  'компьютер': { viewport: { width: 1440, height: 900 }, net: null, cpu: 1 },
};

const rows = [];
const apis = new Map();
for (const [name, p] of Object.entries(PROFILES)) {
  for (const path of PAGES) {
    const ctx = await browser.newContext({ viewport: p.viewport, isMobile: p.isMobile, hasTouch: p.hasTouch, storageState: state });
    await ctx.addInitScript(() => {
      window.__lcp = 0;
      new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__lcp = e.startTime; }).observe({ type: 'largest-contentful-paint', buffered: true });
    });
    const page = await ctx.newPage();
    const cdp = await ctx.newCDPSession(page);
    await cdp.send('Network.enable');
    if (p.net) await cdp.send('Network.emulateNetworkConditions', { offline: false, ...p.net });
    if (p.cpu > 1) await cdp.send('Emulation.setCPUThrottlingRate', { rate: p.cpu });
    let bytes = 0, requests = 0;
    const started = new Map();
    page.on('request', (r) => started.set(r, Date.now()));
    page.on('requestfinished', async (r) => {
      requests++;
      const s = await r.sizes().catch(() => null);
      if (s) bytes += s.responseBodySize + s.responseHeadersSize;
      const u = new URL(r.url());
      if (u.pathname.startsWith('/api/') && !u.pathname.startsWith('/api/img')) {
        const t = r.timing();
        const ms = t.responseEnd > 0 ? t.responseEnd : Date.now() - (started.get(r) ?? Date.now());
        const key = `${u.pathname.replace(/\d+/g, ':id').replace(/tt:id/g, 'tt:id')}`;
        if (p.cpu === 1) apis.set(key, Math.max(apis.get(key) ?? 0, ms));
      }
    });
    const t0 = Date.now();
    await page.goto(base + path, { waitUntil: 'load', timeout: 90_000 }).catch(() => {});
    await page.waitForLoadState('networkidle', { timeout: 30_000 }).catch(() => {});
    const total = Date.now() - t0;
    const m = await page.evaluate(() => {
      const nav = performance.getEntriesByType('navigation')[0];
      const fcp = performance.getEntriesByName('first-contentful-paint')[0]?.startTime ?? 0;
      return { fcp: Math.round(fcp), lcp: Math.round(window.__lcp), dcl: Math.round(nav?.domContentLoadedEventEnd ?? 0) };
    });
    rows.push({ профиль: name, страница: path, 'FCP мс': m.fcp, 'LCP мс': m.lcp, 'до тишины мс': total, запросов: requests, КБ: Math.round(bytes / 1024) });
    await ctx.close();
  }
}
console.table(rows);
console.log('\nСамые медленные ответы API (компьютер, без ограничений), мс:');
console.table([...apis.entries()].sort((a, b) => b[1] - a[1]).slice(0, 15).map(([k, v]) => ({ запрос: k, мс: Math.round(v) })));
await browser.close();
child.kill();
process.exit(0);
