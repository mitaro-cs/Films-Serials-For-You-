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
  - `access` — режимы доступа из интернета (ниже, у CloudPub).
  - `api/` — маршруты (`mod.rs`), `account`, `catalog`, `play`, `admin`.
  - `web.rs` — встроенный интерфейс (rust-embed из `web/build`) и заголовки.
- `web/` — SvelteKit 2 + Svelte 5 (руны), adapter-static, SPA. `lib/api.ts`, `session.svelte.ts`,
  `ui/` (компоненты), `player/`, `admin/`. Иконки — свои пути в `ui/icons.ts`, без библиотек.
  - Главная `/` — каталог-билеты по макету (0.3): `?tab=movies|series|new`, `?status=want|watching|watched`,
    данные — `GET /api/library` (все карточки вкладки, фильтр и поиск — на клиенте, `lib/library.ts`).
    Карточка — `ui/Ticket.svelte` (низ — `titles.accent`, вырезы — кружки цвета страницы, белый билет —
    `status = watched`), меню статуса — одно на страницу (`ui/StatusMenu.svelte`, `cards.svelte.ts`).
  - Статусы: «Хочу» = `my_list`; «Смотрю» — есть прогресс; «Просмотрено» — таблица `watched`
    (вручную `PUT /api/titles/{id}/watched` или само: `catalog::mark_if_finished` после досмотра
    всех файлов). Снять «Просмотрено» — стирает прогресс. `Card.resume` — где остановился (только у «Смотрю»).
  - `titles.accent` — цвет по постеру (`images::accent`, тёмный и приглушённый — белый текст
    читается, проверено тестом), считается вместе с `poster_sm`.
  - Люди (0.4, `api/social.rs`): действуют **профили** (не аккаунты), детские — без общения
    (`SOCIAL`, `adult()`). Отзывы `reviews` (звёзды 1..10 = половинки, средняя — по последней
    оценке человека), `review_likes`, дружба `friends` (одна строка на пару: a просит b), сообщения
    `messages` (только друзьям, `title_id` — фильм карточкой, лимит 30/мин). Фото профиля —
    `profiles.photo` (квадрат 256, `probe::square_image`). Регистрация по умолчанию открытая
    (`account::REGISTRATION_DEFAULT`, `signup_limit` 5 за 10 мин с адреса).
  - Фронт: `lib/social/` (отзывы, звёзды, дружба, чаты, значки — опрос `/api/badges` раз в 30 с),
    страницы `/people`, `/people/[id]`, `/chats`, `/chats/[id]` (опрос `?after=` раз в 3 с).
  - Добавление фильмов (0.4, `library/inbox.rs`): своя папка (`settings media.inbox`, у приложения —
    `Host::media_dir()`), `import()` для перетаскивания в окно (папки — библиотекой, файлы — копия
    через `*.part`), `PUT /api/admin/upload` потоком, `watch_worker` (notify, тишина 3 с → скан).
- `desktop/src-tauri` — Tauri 2: сервер работает в процессе приложения, окно открывает
  `http://127.0.0.1:<порт окна>/api/host/enter?t=…`, трей, `DesktopHost` (выбор папки).
  `update.rs` — автообновление (tauri-plugin-updater, `latest.json` выпуска, подпись — секрет
  `TAURI_SIGNING_PRIVATE_KEY`, открытый ключ — в tauri.conf.json; без секрета `release.yml`
  собирает с `no-updater.conf.json`). Перед установкой — `stop_server`.
- `access` — `cloudpub` (рекомендуемый, `cloudpub.rs`: клиент `clo` по SHA-256, `register` один
  раз → постоянный адрес, `run`; вход почтой/паролем или ключом API, пароль не хранится),
  `fxtunnel`, `custom`, `lan`.
- Значок — пиксельная хлопушка: `python3 scripts/icon.py png` (SVG/PNG для сайта и `icon-src`),
  затем `npm run icons` в `desktop/`.

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
- Выпуск: тег `vX.Y.Z` = `version` в `tauri.conf.json` (иначе `release.yml` останавливается
  сразу). В `tauri-action` нет `tauriScript`: с `npm run tauri --` он сам добавлял ещё один `--`,
  `--target` уходил в cargo, а упаковщик искал программу в `target/release` (0.1.1 так упал).
- Mac: бандл подписывается ad-hoc (`bundle.macOS.signingIdentity: "-"`), ffmpeg — `codesign -s -`
  в `fetch-ffmpeg.sh`; без подписи Apple Silicon пишет «приложение повреждено» (0.3.0).
- hls.js — только `hls.js/light` и только лениво из плеера (это ~120 КБ gzip).
- Внешних запросов из фронта нет (шрифты системные, иконки свои); картинки IMDb/TMDB сервер
  скачивает к себе (`images/<sha256>.ext`).
- Коммиты — Conventional Commits (`feat:`, `fix:`, …).
- Ветка `miaro` — основная; владелец поручил Claude вести её самому: работа — в своей ветке →
  PR в `miaro` → зелёная CI (server, web, e2e, desktop) → Claude сливает PR сам (merge commit).
  Прямо в `miaro` не пушить. Теги выпусков — только по слову владельца.

## Проверки перед пушем

`cargo fmt --all --check`, `cargo clippy -p fsfy-server --all-targets -- -D warnings`,
`cargo test -p fsfy-server` (сквозные тесты `server/tests/*.rs` нужен ffmpeg),
в `web/`: `npm run check`, `npm test`, `npm run build`, затем `cargo build --release -p fsfy-server`
и `npx playwright test` (e2e: демо-данные из `e2e/setup.ts`, телефон/планшет/компьютер).

- `e2e/util.ts expectClean` — вёрстка: страница не шире экрана (называет виновника), строки
  текста и кнопки не пересекаются. Новую страницу — в `e2e/layout.spec.ts`. Ошибка JS — провал
  (`util-test.ts`). Бюджет JS главной — `e2e/speed.spec.ts` (80 КБ сжатого).
- Сетки с прокручиваемыми лентами — `minmax(0, 1fr)`, не `1fr` (иначе лента распирает страницу).
- Ключи в `{#each}` — уникальные (одинаковые кадры серий уронили страницу сериала).
- «Ничего не дёргается»: `e2e/smooth.spec.ts` — CLS < 0,02 на главных страницах. Не показывать
  то, что вырастет после загрузки (числа в капсулах, отзывы — грузить вместе со страницей),
  `main` — не ниже экрана. Анимации — только `transform`/`opacity`.
- Навигация: ≥1000 px — капсула в шапке, уже — нижняя панель (`--tabbar` — отступ под неё).
- e2e, которые меняют дружбу или статусы, заводят своего человека (`stranger()`) или
  сбрасывают состояние (`resetStatuses`): проекты phone/tablet/desktop идут подряд на одних данных.
