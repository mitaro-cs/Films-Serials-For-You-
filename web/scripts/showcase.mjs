// Скриншоты для README на настоящих данных: `node scripts/showcase.mjs` (из web/, после
// `cargo build --release -p fsfy-server`). Нужны ffmpeg и интернет - описания, постеры и кадры
// сервер берёт с IMDb сам, как у хоста.
// Фильмы - те, что можно показывать: открытые фильмы Blender (CC BY) и классика в общественном
// достоянии. Видео в папках - короткие заглушки (на скриншотах видны постеры и кадры с IMDb).
import { chromium, request } from '@playwright/test';
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '../..');
const OUT = join(ROOT, 'docs/screenshots');

const MOVIES = [
  'Sintel (2010)',
  'Tears of Steel (2012)',
  'Big Buck Bunny (2008)',
  'Cosmos Laundromat (2015)',
  'Elephants Dream (2006)',
  'Spring (2019)',
  'Sprite Fright (2021)',
  'Charge (2022)',
  'Agent 327 Operation Barbershop (2017)',
  'Night of the Living Dead (1968)',
  'Nosferatu (1922)',
  'Metropolis (1927)',
  'The General (1926)',
  'His Girl Friday (1940)',
  'Charade (1963)',
  'The Kid (1921)',
];
const SERIES = { dir: 'Pioneer One (2010)', episodes: 4 };

function ff(args) {
  execFileSync('ffmpeg', ['-y', '-hide_banner', '-loglevel', 'error', ...args]);
}

// 20 секунд тёмного кадра со звуком - чтобы было что «начать смотреть».
function video(path) {
  mkdirSync(dirname(path), { recursive: true });
  ff(['-f', 'lavfi', '-i', 'color=c=0x101014:s=640x360:r=24:d=20', '-f', 'lavfi', '-i', 'anullsrc=r=48000:cl=stereo',
    '-t', '20', '-c:v', 'libx264', '-preset', 'ultrafast', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest', path]);
}

function binary() {
  for (const p of [process.env.FSFY_BIN, join(ROOT, 'target/release/fsfy.exe'), join(ROOT, 'target/release/fsfy')]) {
    if (p && existsSync(p)) return p;
  }
  throw new Error('Нет сервера: соберите `cargo build --release -p fsfy-server`');
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const base = mkdtempSync(join(tmpdir(), 'nova-showcase-'));
const media = join(base, 'media');
for (const dir of MOVIES) video(join(media, dir, `${dir.replace(/ \(\d+\)$/, '')}.mp4`));
for (let e = 1; e <= SERIES.episodes; e++) video(join(media, SERIES.dir, 'Season 1', `Pioneer.One.S01E0${e}.mp4`));

const port = 19500 + Math.floor(Math.random() * 300);
const win = port + 1000;
const child = spawn(binary(), ['serve', '--data', join(base, 'data'), '--port', String(port), '--window-port', String(win)], {
  env: { ...process.env, RUST_LOG: 'warn' },
  stdio: ['pipe', 'pipe', 'inherit'],
});
const stop = () => {
  child.kill();
  setTimeout(() => rmSync(base, { recursive: true, force: true }), 1500);
};
process.on('exit', () => child.kill());

try {
  const entry = await new Promise((ok, fail) => {
    let out = '';
    const t = setTimeout(() => fail(new Error(`сервер не запустился: ${out}`)), 30_000);
    child.stdout.on('data', (d) => {
      out += d;
      const m = out.match(/Вход хоста:\s+(\S+)/);
      if (m) {
        clearTimeout(t);
        ok(m[1]);
      }
    });
  });

  const host = `http://127.0.0.1:${win}`;
  const api = await request.newContext({ baseURL: host, extraHTTPHeaders: { 'x-fsfy': '1' } });
  await api.get(entry);
  const send = async (method, path, data) => {
    const r = await api.fetch(path, { method, data });
    if (!r.ok()) throw new Error(`${method} ${path}: ${r.status()} ${await r.text()}`);
    return r.json().catch(() => null);
  };
  const until = async (f, what, tries = 600) => {
    for (let i = 0; i < tries; i++) {
      if (await f()) return;
      await sleep(500);
    }
    throw new Error(`не дождались: ${what}`);
  };

  await send('POST', '/api/admin/libraries', { path: media });
  const total = MOVIES.length + 1;
  await until(async () => (await send('GET', '/api/admin/titles')).length === total, 'скан');
  console.log('Описания с IMDb…');
  await until(async () => {
    const s = await send('GET', '/api/admin/status');
    return s.metaPending === 0 && s.unprobed === 0;
  }, 'описания');
  const titles = await send('GET', '/api/admin/titles');
  const missing = titles.filter((t) => !t.poster).map((t) => t.title);
  if (missing.length) console.warn('Без постера:', missing.join(', '));
  // Кадры (1920 и 640 px), лёгкие постеры и цвет карточек качаются и считаются в фоне - ждём,
  // пока число готовых карточек не перестанет расти.
  const all = async () => [
    ...(await send("GET", "/api/library?tab=movies")).items,
    ...(await send("GET", "/api/library?tab=series")).items,
  ];
  const ready = (c) => c.backdrop && c.backdropSmall && /[а-яё]/i.test(c.overview ?? "") && c.poster;
  let seen = -1;
  let calm = 0;
  for (let i = 0; i < 90 && calm < 8; i++) {
    const n = (await all()).filter(ready).length;
    calm = n === seen ? calm + 1 : 0;
    seen = n;
    await sleep(2000);
  }
  const cards = await all();
  const rich = cards.filter(ready);
  console.log(`Готово карточек: ${rich.length} из ${cards.length}`);
  // Порядок - у кого кадры с IMDb светлые и красивые (у части классики кадр почти чёрный).
  const ORDER = [/Tears of Steel/i, /The Kid/i, /Sintel/i, /Elephants Dream/i, /Big Buck Bunny/i, /Cosmos Laundromat/i, /Sprite Fright/i, /Agent 327/i, /Nosferatu/i, /Charade/i];
  const rank = (c) => {
    const k = ORDER.findIndex((re) => re.test(c.originalTitle ?? "") || re.test(c.title));
    return k < 0 ? ORDER.length : k;
  };
  const films = rich.filter((c) => c.kind === "movie").sort((a, b) => rank(a) - rank(b));
  const movie = films[0];
  const show = rich.find((c) => c.kind === "series") ?? cards.find((c) => c.kind === "series");
  const others = films.slice(1);

  // Статусы и место просмотра - как у живого человека.
  const progress = async (t, part, ep = 0) => {
    const files = (await send("GET", `/api/titles/${t}`)).files;
    await send("PUT", `/api/progress/${files[Math.min(ep, files.length - 1)].id}`, { position: 20 * part, duration: 20 });
  };
  await progress(movie.id, 0.4);
  await progress(show.id, 0.6, 1);
  if (others[0]) await progress(others[0].id, 0.25);
  for (const c of others.slice(1, 3)) await send("PUT", `/api/titles/${c.id}/status`, { status: "want" });
  for (const c of others.slice(3, 5)) await send("PUT", `/api/titles/${c.id}/status`, { status: "watched" });

  // Друзья, отзывы и переписка.
  const pub = `http://127.0.0.1:${port}`;
  const person = async (name, login) => {
    await send('POST', '/api/admin/users', { name, login, password: 'showcase-12345' });
    const c = await request.newContext({ baseURL: pub, extraHTTPHeaders: { 'x-fsfy': '1' } });
    await c.post('/api/auth/login', { data: { login, password: 'showcase-12345' } });
    const me = await (await c.get('/api/me')).json();
    return { c, id: me.user.profile.id };
  };
  const anna = await person('Анна', 'anna');
  const max = await person('Максим', 'max');
  const me = (await send('GET', '/api/me')).user.profile.id;
  const sintel = movie.id;
  if (sintel) {
    await anna.c.post(`/api/titles/${sintel}/reviews`, { data: { stars: 9, body: 'Короткий, но целая история. Финал до сих пор вспоминаю.' } });
    await max.c.post(`/api/titles/${sintel}/reviews`, { data: { stars: 8, body: 'Красиво снято, музыка отличная. Хочется продолжения.' } });
  }
  for (const p of [anna, max]) {
    await p.c.post(`/api/friends/${me}`);
    await send('POST', `/api/friends/${p.id}`);
  }
  await anna.c.post(`/api/chats/${me}`, { data: { body: `Посмотрела «${movie.title}» - смотрим вечером вместе?`, titleId: sintel } });
  await send('POST', `/api/chats/${anna.id}`, { body: 'Давай! Позову Максима 🙂' });
  await anna.c.dispose();
  await max.c.dispose();

  // Снимки: компьютер и телефон, без заставки (её пропускают в тестах - navigator.webdriver).
  mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch();
  const state = await api.storageState();
  const shoot = async (viewport, scale, shots) => {
    const ctx = await browser.newContext({ viewport, deviceScaleFactor: scale, storageState: state, isMobile: viewport.width < 600, hasTouch: viewport.width < 600 });
    // Трейлеры (H.264) встроенный Chromium Playwright не играет - был бы чёрный кадр; без них виден кадр фильма.
    await ctx.route('**/trailer**', (r) =>
      r.fulfill({ status: 404, contentType: 'application/json', headers: { 'x-fsfy': '1' }, body: '{"error":"not_found","message":"Нет"}' }),
    );
    const page = await ctx.newPage();
    for (const [path, name, prep] of shots) {
      await page.goto(host + path);
      await page.waitForLoadState('networkidle');
      if (prep) await prep(page);
      await sleep(1500);
      const png = join(base, `${name}.png`);
      await page.screenshot({ path: png });
      ff(['-i', png, '-c:v', 'libwebp', '-quality', '82', join(OUT, `${name}.webp`)]);
      console.log('Снимок', name);
    }
    await ctx.close();
  };
  const pioneer = show.id;
  await shoot({ width: 1440, height: 900 }, 1, [
    ['/', '1-home'],
    [`/title/${sintel}`, '2-movie'],
    [`/title/${pioneer}`, '3-series'],
    ['/movies', '4-movies'],
    [`/chats/${anna.id}`, '5-chat'],
    ['/admin', '6-admin'],
  ]);
  await shoot({ width: 390, height: 844 }, 2, [
    ['/', '7-phone-home'],
    [`/title/${sintel}`, '8-phone-movie'],
    [`/title/${pioneer}`, '9-phone-series'],
  ]);
  await browser.close();
  await api.dispose();
  console.log('Готово:', OUT);
} finally {
  stop();
}
