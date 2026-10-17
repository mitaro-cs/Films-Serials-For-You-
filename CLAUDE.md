# Films & Serials For You — заметки для Claude Code

Домашний кинотеатр: приложение хоста (Mac, Windows) раздаёт фильмы и сериалы со своего диска,
зрители смотрят в браузере. Всё на русском: интерфейс, документация, ошибки API.
Видимое имя — «Films & Serials For You»; внутреннее — `fsfy` (крейт, бинарник, `app.fsfy` —
каталог данных, cookie `fsfy_*`, заголовок `X-Fsfy`). Внутренние имена не менять — сломаются данные.

## Устройство

- `server/` — крейт `fsfy-server` (библиотека `fsfy` + бинарник `fsfy serve`): axum 0.8, tokio,
  rusqlite (одно соединение под мьютексом, WAL), reqwest (rustls).
  - `library/` — `parse` (имя файла → название, год, сезон/серия; папка «Название (год)»
    главнее имени файла), `scan` (обход, `missing` вместо удаления), `probe` (ffprobe с главами,
    `chapter_markers`, кадры перемотки, уменьшение постеров), `jobs` (обход, ffprobe, потом
    лёгкие постеры `poster_sm` и кадры перемотки `files.trickplay`).
  - `meta/` — `imdb` (по умолчанию, без ключа: подсказки + GraphQL сайта IMDb, неофициально),
    `tmdb`, `kp`; `apply` скачивает картинки и пишет описание; `worker` — очередь `pending`.
  - `stream` — Range, перепаковка ffmpeg в фрагментированный MP4 (`?t=` — с места), субтитры
    в WebVTT (cp1251 → UTF-8).
  - `auth` — Argon2id, сессии (в базе — SHA-256 токена), `Viewer`/`Manager`, `Limiter`.
    У `Viewer` всегда есть `profile` (сессия помнит выбранный, `sessions.profile_id`); прогресс,
    `my_list`, `ratings` — по `profile_id`. Детский профиль — `api::visible(kids)` (`KIDS`:
    `titles.age_min` ≤ 12 или жанры мультфильмов).
  - `taste` — вкус профиля: веса жанров → `match_pct`, «не для меня», `age_min()`.
  - `hls` — адаптивный поток: плейлист заранее (кусочки по 6 с), ffmpeg с `-ss`,
    `-force_key_frames` на границах и `-output_ts_offset`; задача на (файл, высота, дорожка),
    `janitor` останавливает убежавшие вперёд и брошенные.
  - `access` — режимы `lan` / `fxtunnel` (клиент с GitHub по SHA-256) / `custom`.
  - `api/` — маршруты (`mod.rs`), `account`, `catalog`, `play`, `admin`.
  - `web.rs` — встроенный интерфейс (rust-embed из `web/build`) и заголовки.
- `web/` — SvelteKit 2 + Svelte 5 (руны), adapter-static, SPA. `lib/api.ts`, `session.svelte.ts`,
  `ui/` (компоненты), `player/`, `admin/`. Иконки — свои пути в `ui/icons.ts`, без библиотек.
- `desktop/src-tauri` — Tauri 2: сервер работает в процессе приложения, окно открывает
  `http://127.0.0.1:<порт окна>/api/host/enter?t=…`, трей, `DesktopHost` (выбор папки).

## Соглашения

- Два порта: общий (`0.0.0.0`, по умолчанию 17480) и окна хоста (`127.0.0.1`, 17481).
  Сессия окна (`fsfy_w`, `sessions.local`) действует **только** через порт окна: туннель тоже
  приходит с 127.0.0.1. Метка входа — `auth::Via` (слой на роутер каждого порта).
- Изменяющие запросы `/api/*` требуют заголовок `X-Fsfy: 1` (CSRF), фронт ставит сам.
- Ответы помечены `X-Fsfy: 1`: без метки (страница туннеля) фронт считает сервер выключенным.
- Ошибки API: `{"error": "код", "message": "по-русски"}` через `ApiError`.
- Время — миллисекунды Unix. Миграции — новый элемент `db::MIGRATIONS`, старые не править
  (после первого выпуска).
- Права: `Viewer` — вошёл, `Manager` — хост или администратор; назначать администраторов — хост.
- Никаких `confirm()/alert()`: в окне приложения на Mac (WKWebView) они не показываются —
  `ui/dialogs.svelte.ts`.
- Имена файлов не должны совпадать без учёта регистра (`x.svelte.ts` и `X.svelte` — тоже):
  на Mac и Windows импорт находит не тот файл (сборка 0.1.0 так упала). Ловит
  `web/src/lib/paths.test.ts`.
- hls.js — только `hls.js/light` и только лениво из плеера (это ~120 КБ gzip).
- Внешних запросов из фронта нет (шрифты системные, иконки свои); картинки IMDb/TMDB сервер
  скачивает к себе (`images/<sha256>.ext`).
- Коммиты — Conventional Commits (`feat:`, `fix:`, …).

## Проверки перед пушем

`cargo fmt --all --check`, `cargo clippy -p fsfy-server --all-targets -- -D warnings`,
`cargo test -p fsfy-server` (сквозной тест `server/tests/flow.rs` нужен ffmpeg),
в `web/`: `npm run check`, `npm test`, `npm run build`.
