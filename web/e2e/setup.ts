import { request, type FullConfig } from '@playwright/test';
import { execFileSync, spawn } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';

// Демо-библиотека: короткие видео и постеры разных цветов - как на макете, плюс крайние случаи
// (очень длинное название, без страны и режиссёра, сериал). Хост входит одноразовой ссылкой
// окна; статусы «Хочу», «Смотрю» и «Просмотрено» расставлены заранее.

const ROOT = resolve(import.meta.dirname, '../..');

export const MOVIES = [
  { dir: 'Ла-Ла Ленд (2016)', c: ['0x6b3fa0', '0x1f3a8a'], country: 'США', director: 'Дэмьен Шазелл', genres: ['Мюзикл', 'Драма', 'Мелодрама'] },
  { dir: 'Начало (2010)', c: ['0x1f6f78', '0x0b2530'], country: 'США', director: 'Кристофер Нолан', genres: ['Фантастика', 'Боевик'] },
  { dir: 'Интерстеллар (2014)', c: ['0x24314f', '0x8a6a3c'], country: 'Великобритания', director: 'Кристофер Нолан', genres: ['Фантастика', 'Драма'] },
  { dir: 'Амели (2001)', c: ['0x2f6b2f', '0xa8322d'], country: 'Франция', director: 'Жан-Пьер Жёне', genres: ['Комедия', 'Мелодрама'] },
  { dir: 'Паразиты (2019)', c: ['0x5a5a5a', '0x202020'], country: 'Южная Корея', director: 'Пон Джун-хо', genres: ['Триллер', 'Драма'] },
  { dir: 'Дюна (2021)', c: ['0xc8752a', '0x3a1f0c'], country: 'США', director: 'Дени Вильнёв', genres: ['Фантастика', 'Приключения'] },
  { dir: 'Очень длинное название фильма, чтобы проверить переносы строк на узком телефоне (2020)', c: ['0x8a2be2', '0xff7f50'], country: 'Соединённые Штаты Америки', director: 'Режиссёр С Очень Длинным Именем И Фамилией', genres: ['Драма', 'Комедия', 'Фэнтези', 'Мультфильм'] },
  { dir: 'Без описания (2022)', c: ['0xdedede', '0xf5f5f5'], country: '', director: '', genres: [] },
];

function ff(args: string[]) {
  execFileSync('ffmpeg', ['-y', '-hide_banner', '-loglevel', 'error', ...args]);
}

function video(path: string) {
  mkdirSync(dirname(path), { recursive: true });
  ff(['-f', 'lavfi', '-i', 'testsrc2=size=320x180:rate=24:duration=6', '-f', 'lavfi', '-i', 'sine=f=440:duration=6',
    '-c:v', 'libx264', '-preset', 'ultrafast', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest', path]);
}

function poster(path: string, [c0, c1]: string[]) {
  ff(['-f', 'lavfi', '-i', `gradients=s=400x600:c0=${c0}:c1=${c1}:x0=0:y0=0:x1=400:y1=600:d=1`, '-frames:v', '1', path]);
}

function binary(): string {
  const env = process.env.FSFY_BIN;
  for (const p of [env, join(ROOT, 'target/release/fsfy'), join(ROOT, 'target/debug/fsfy')]) {
    if (p && existsSync(p)) return p;
  }
  throw new Error('Нет сервера: соберите `cargo build --release -p fsfy-server` (или задайте FSFY_BIN)');
}

export default async function setup(_: FullConfig) {
  const base = mkdtempSync(join(tmpdir(), 'fsfy-e2e-'));
  const media = join(base, 'media');
  for (const m of MOVIES) {
    const name = m.dir.replace(/ \(\d+\)$/, '');
    video(join(media, m.dir, `${name}.mp4`));
    poster(join(media, m.dir, 'poster.jpg'), m.c);
  }
  for (const e of [1, 2]) video(join(media, 'Тьма', 'Season 1', `Тьма.s01e0${e}.mp4`));
  poster(join(media, 'Тьма', 'poster.jpg'), ['0x0e3b2e', '0x050505']);

  const port = 18000 + Math.floor(Math.random() * 1000);
  const win = port + 1000;
  const child = spawn(binary(), ['serve', '--data', join(base, 'data'), '--port', String(port), '--window-port', String(win)], {
    env: { ...process.env, RUST_LOG: 'warn' },
    stdio: ['pipe', 'pipe', 'inherit'],
  });
  const entry = await new Promise<string>((ok, fail) => {
    let out = '';
    const t = setTimeout(() => fail(new Error(`сервер не запустился: ${out}`)), 30_000);
    child.stdout!.on('data', (d) => {
      out += d;
      const m = out.match(/Вход хоста:\s+(\S+)/);
      if (m) {
        clearTimeout(t);
        ok(m[1]);
      }
    });
    child.on('exit', (c) => fail(new Error(`сервер завершился (${c}): ${out}`)));
  });

  const baseURL = `http://127.0.0.1:${win}`;
  const api = await request.newContext({ baseURL, extraHTTPHeaders: { 'x-fsfy': '1' } });
  await api.get(entry);
  const send = async (method: string, path: string, data?: unknown) => {
    const r = await api.fetch(path, { method, data });
    if (!r.ok()) throw new Error(`${method} ${path}: ${r.status()} ${await r.text()}`);
    return r.json().catch(() => null);
  };
  const until = async (f: () => Promise<boolean>, what: string) => {
    for (let i = 0; i < 300; i++) {
      if (await f()) return;
      await new Promise((r) => setTimeout(r, 200));
    }
    throw new Error(`не дождались: ${what}`);
  };

  await send('PUT', '/api/admin/settings', { provider: 'off' });
  await send('POST', '/api/admin/libraries', { path: media });
  await until(async () => (await send('GET', '/api/titles')).length === MOVIES.length + 1, 'скан');
  await until(async () => (await send('GET', '/api/admin/status')).unprobed === 0, 'ffprobe');
  const titles: { id: number; title: string }[] = await send('GET', '/api/admin/titles');
  const id = (t: string) => titles.find((x) => x.title === t)!.id;
  for (const m of MOVIES) {
    const t = m.dir.replace(/ \(\d+\)$/, '');
    await send('PATCH', `/api/admin/titles/${id(t)}`, { genres: m.genres, countries: [m.country], director: m.director });
  }
  await send('PATCH', `/api/admin/titles/${id('Тьма')}`, { genres: ['Триллер', 'Фантастика'], countries: ['Германия'], director: 'Баран бо Одар' });
  // Цвет карточек считается фоном после постеров.
  await until(async () => {
    const l = await send('GET', '/api/library?tab=movies');
    return l.items.every((c: { accent: string | null }) => c.accent);
  }, 'цвет карточек');

  // Статусы: хочу, смотрю (фильм и сериал), просмотрено.
  await send('PUT', `/api/list/${id('Интерстеллар')}`, { value: true });
  await send('PUT', `/api/list/${id('Дюна')}`, { value: true });
  await send('PUT', `/api/titles/${id('Ла-Ла Ленд')}/watched`, { value: true });
  await send('PUT', `/api/titles/${id('Паразиты')}/watched`, { value: true });
  const nach = await send('GET', `/api/titles/${id('Начало')}`);
  await send('PUT', `/api/progress/${nach.files[0].id}`, { position: 2.5, duration: 6 });
  const dark = await send('GET', `/api/titles/${id('Тьма')}`);
  await send('PUT', `/api/progress/${dark.files[0].id}`, { position: 6, duration: 6 });

  // Люди: Анна и Борис (аккаунты заводит хост - лимит самостоятельных регистраций с одного
  // адреса оставляем тестам регистрации), отзывы, дружба, переписка.
  const publicURL = `http://127.0.0.1:${port}`;
  const signup = async (name: string, login: string) => {
    await send('POST', '/api/admin/users', { name, login, password: 'пароль-12345' });
    const c = await request.newContext({ baseURL: publicURL, extraHTTPHeaders: { 'x-fsfy': '1' } });
    const r = await c.post('/api/auth/login', { data: { login, password: 'пароль-12345' } });
    if (!r.ok()) throw new Error(`вход ${login}: ${r.status()} ${await r.text()}`);
    const me = await (await c.get('/api/me')).json();
    return { c, id: me.user.profile.id as number };
  };
  const anna = await signup('Анна Смирнова', 'anna');
  const boris = await signup('Борис', 'boris');
  const me = (await send('GET', '/api/me')).user.profile.id as number;
  const lala = id('Ла-Ла Ленд');
  await anna.c.post(`/api/titles/${lala}/reviews`, { data: { stars: 9, body: 'Музыка, цвета и финал, который разбивает сердце. Пересматривала трижды.' } });
  await boris.c.post(`/api/titles/${lala}/reviews`, { data: { stars: 7, body: 'Красиво, но середина затянута.' } });
  await send('POST', `/api/titles/${lala}/reviews`, { stars: 10, body: 'Лучший мюзикл десятилетия.' });
  await anna.c.post(`/api/friends/${me}`);
  await send('POST', `/api/friends/${anna.id}`);
  await anna.c.post(`/api/chats/${me}`, { data: { body: 'Привет! Посмотрела «Дюну» - давай обсудим?', titleId: id('Дюна') } });
  await send('POST', `/api/chats/${anna.id}`, { body: 'Давай! Мне больше понравилась музыка 🙂' });
  await boris.c.post(`/api/friends/${me}`);
  await anna.c.dispose();
  await boris.c.dispose();

  // Фильм из ленты IMDb со своей страницей (/ext/…): без сети — описание, кадр, актёры и похожие
  // кладём прямо в базу (`detailed` = 5 = `discover::DETAILED` - сервер к IMDb не пойдёт), картинки — в images/.
  const data = join(base, 'data');
  const img = (src: string) => {
    const name = `${createHash('sha256').update(readFileSync(src)).digest('hex')}.jpg`;
    copyFileSync(src, join(data, 'images', name));
    return name;
  };
  const art = img(join(media, 'Дюна (2021)', 'poster.jpg'));
  const wide = join(base, 'wide.jpg');
  ff(['-f', 'lavfi', '-i', 'gradients=s=1280x720:c0=0x1f3a8a:c1=0xc8752a:x0=0:y0=0:x1=1280:y1=720:d=1', '-frames:v', '1', wide]);
  const still = img(wide);
  const db = new DatabaseSync(join(data, 'fsfy.db'));
  db.exec('PRAGMA busy_timeout = 5000');
  const put = db.prepare(
    `INSERT OR REPLACE INTO ext_titles (id, kind, title, original, year, rating, votes, genres, overview, runtime, poster, backdrop, extra, detailed, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  );
  const similar = ['tt0000001', 'tt0000002', 'tt0000003'];
  similar.forEach((s, i) => put.run(s, 'series', `Похожий сериал ${i + 1}`, null, 2010 + i, 7.5, 1000, '["Драма"]', null, null, art, null, JSON.stringify({ at: Date.now(), imdbBackdrop: art }), 5, Date.now()));
  const extra = {
    // Кадр IMDb уже есть - страница не идёт за ним в сеть и не затирает описание и актёров из базы.
    imdbBackdrop: still,
    tagline: 'Он читает между строк',
    age: '16+',
    countries: ['США'],
    languages: ['Английский'],
    releaseDate: '2008-09-23',
    aka: [],
    poster: art,
    people: [
      { name: 'Бруно Хеллер', job: 'creator', role: null, photo: null },
      ...Array.from({ length: 14 }, (_, k) => ({ name: `Актёр С Довольно Длинным Именем ${k + 1}`, job: 'actor', role: `Персонаж ${k + 1}`, photo: k % 2 ? art : null })),
    ],
    gallery: [still, still, still],
    similar,
    at: Date.now(),
  };
  put.run('tt1196946', 'series', 'Менталист', 'The Mentalist', 2008, 8.2, 257674, '["Криминал","Драма","Детектив"]',
    'Известный «экстрасенс» Патрик Джейн признаётся, что он мошенник, и помогает полиции искать Красного Джона.', 43, art, still, JSON.stringify(extra), 5, Date.now());
  db.prepare("INSERT OR REPLACE INTO ext_charts (chart, pos, ext_id) VALUES ('popular_series', 0, 'tt1196946')").run();
  db.close();
  // Своё видео у фильма — страница не ищет его на Rutube и VK (в CI это была бы сеть).
  await send('POST', '/api/links', { url: 'https://example.com/mentalist-1.mp4', extId: 'tt1196946', label: 'Менталист, 1 серия' });

  await api.storageState({ path: join(import.meta.dirname, '.state.json') });
  await api.dispose();
  writeFileSync(
    join(import.meta.dirname, '.env.json'),
    JSON.stringify({ baseURL, publicURL, ids: Object.fromEntries(titles.map((t) => [t.title, t.id])), people: { me, anna: anna.id, boris: boris.id } }),
  );

  return async () => {
    // На Windows сервер отпускает файлы не сразу после kill: ждём выхода и удаляем с повторами.
    const gone = new Promise((r) => child.once('exit', r));
    child.kill();
    await Promise.race([gone, new Promise((r) => setTimeout(r, 5000))]);
    try {
      rmSync(base, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
    } catch (e) {
      console.warn(`не удалось убрать ${base}: ${e}`);
    }
  };
}
