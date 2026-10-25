# Nova (бывший Films & Serials For You) — заметки для Claude Code

Домашний кинотеатр: приложение хоста (Mac, Windows) раздаёт фильмы и сериалы со своего диска,
зрители смотрят в браузере. Всё на русском: интерфейс, документация, ошибки API.
Видимое имя — «Nova» (с 0.6, выбор владельца; `productName` приложения — тоже, `identifier`
остался `app.fsfy`); внутреннее — `fsfy` (крейт, бинарник, `app.fsfy` — каталог данных, cookie
`fsfy_*`, заголовок `X-Fsfy`). Внутренние имена не менять — сломаются данные.

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
    в WebVTT (cp1251 → UTF-8); одинаковые подписи дорожек — «Русский 2» (`distinct`).
  - `copy` (0.5) — «Оригинал» для не-MP4 (H.264 8 бит, HEVC): одна копия ffmpeg `-c:v copy` в
    HLS из fMP4 (`cache/copy`, ~50–100× быстрее просмотра), `GET /api/files/{id}/copy/{audio}/…`.
    Не возвращать потоковую перепаковку в плеер: Safari (окно на Mac) ходит по Range, и на каждый
    запрос стартовал новый ffmpeg — видео обрывалось «Видео не воспроизводится» (0.4).
    Копия — только там, где HLS родной (Safari, WebKit); Chromium/WebView2 (Windows) играет
    потоковую перепаковку `/play` — hls.js с копией на Windows падал «Видео не загружается» (0.6).
    ffmpeg копии пишет относительные имена (`current_dir`) — пути Windows ломали плейлист.
  - `auth` — Argon2id, сессии (в базе — SHA-256 токена), `Viewer`/`Manager`, `Limiter`.
    У `Viewer` всегда есть `profile` — с 0.5 один на аккаунт (`profile_for`: первый по id;
    миграция 5 удалила лишние). Прогресс, `my_list`, `ratings`, общение — по `profile_id`;
    имя профиля = имя аккаунта (`PATCH /api/me` меняет оба). `kids` и `api::visible(kids)`
    остались в коде, но включить детский режим больше нечем.
  - `taste` — вкус профиля: веса жанров → `match_pct`, «не для меня», `age_min()`.
  - Фоновый ffmpeg — `probe::background` (nice 19 / IDLE на Windows; просмотр — `command()`,
    nice 10 / «ниже среднего»), кадры перемотки — только у
    начатого, `-threads 1`; перекодирование HLS — `-threads 4`. Иначе Mac хоста гудит часами.
  - `hls` — адаптивный поток: плейлист заранее (кусочки по 6 с), ffmpeg с `-ss`,
    `-force_key_frames` на границах и `-output_ts_offset`; задача на (файл, высота, дорожка),
    `janitor` останавливает убежавшие вперёд и брошенные.
  - `access` — режимы доступа из интернета (ниже, у CloudPub).
  - `api/` — маршруты (`mod.rs`), `account`, `catalog`, `play`, `admin`.
  - `web.rs` — встроенный интерфейс (rust-embed из `web/build`) и заголовки.
- `web/` — SvelteKit 2 + Svelte 5 (руны), adapter-static, SPA. `lib/api.ts`, `session.svelte.ts`,
  `ui/` (компоненты), `player/`, `admin/`. Иконки — свои пути в `ui/icons.ts`, без библиотек.
  - Главная `/` — как у Apple TV (0.5): `ui/Hero.svelte` (карусель `featured()`, 8 с, свайп,
    точки), ряды `ui/Shelf.svelte` из `shelves()` (`lib/library.ts`), карточка `ui/Wide.svelte`
    (кадр 16:9 `backdropSmall` = `titles.backdrop_sm` 640 px, подпись `caption()`).
    `?tab=movies|series` — тот же вид + фильтр `?status=` и сетка. Каталог — один запрос
    `GET /api/library` на все вкладки, держится в `lib/catalog.svelte.ts` (обновляется не чаще 15 с).
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
  - Совместные списки (0.5, `api/lists.rs`): `lists`, `list_members`, `list_items` (миграция 5),
    звать — только друзей, менять — всем участникам, удаляет создатель (участник — выходит).
    Фронт — `/lists`, `/lists/[id]`, `social/ListPicker` («В общий список» на странице фильма).
    С 0.6 (миграция 6): фильмы из ленты — `list_ext`, группы — `list_groups` + `group_id`
    (`PUT /api/lists/{id}/move`), `/lists` сверху — «Буду смотреть»/«Просмотрел» (`/api/me/marks`).
  - Обзор (0.6, `discover.rs` + `api/discover.rs`): подборки IMDb (`CHARTS`, GraphQL
    `chartTitles`, раз в 12 ч, `ext_titles` + `ext_charts`, постеры — к себе), поиск по IMDb
    (`/api/discover/search`), отметки `ext_marks` (want|watched), просьбы хосту `wishes`
    (`/api/admin/wishes`, значок `badges.wishes`; уходят сами, когда фильм с тем же `imdb_id`
    появился). Фронт — `/discover`, `discover/Poster` (и для фильмов с сервиса: `local`),
    `PosterRow`, страница `/ext/[id]` (с 0.6.4 вместо окна; `remember(card)` — показать сразу,
    `extSheet.version` — перезагрузить списки).
  - Ссылки (0.6, `api/links.rs`): `links` у фильма (`title_id` или `ext_id`), `parse` —
    rutube/vk/youtube → `embed` (iframe; домены — `frame-src` в CSP `web.rs`), прямые
    .mp4/.webm/.m3u8 — наш `<video>` (hls.js лениво). Поиск: Rutube без ключа, VK —
    `settings vk.token`. Страница `/watch/link/[id]`, окно `links/LinkFinder`.
  - Профиль (0.6, `api/profile.rs`): `stats` (часы, фильмы, серии, жанры, 12 месяцев) и
    оформление `profiles.cover|accent|bio|favorites` (`PATCH /api/me/profile`, `/api/me/cover`).
    Фронт — `social/ProfileStats`, `profile/DecorEditor`.
  - Обновление без `latest.json` (0.6, `release.rs`): последний выпуск через GitHub API,
    установщик по ОС/архитектуре — `update.rs install_manual`.
  - Добавление фильмов (0.4, `library/inbox.rs`): своя папка (`settings media.inbox`, у приложения —
    `Host::media_dir()`), `import()` для перетаскивания в окно (папки — библиотекой, файлы — копия
    через `*.part`), `PUT /api/admin/upload` потоком (`folder` — с вложенными, `more=1` — скан
    после последнего файла), `watch_worker` (notify, тишина 3 с → скан). Скан пропускает папку,
    где есть `*.part`; папка с ≥2 видео «01. …»/«Серия 5» — сериал (`scan::numbered_dir`).
- `desktop/src-tauri` — Tauri 2: сервер работает в процессе приложения, окно открывает
  `http://127.0.0.1:<порт окна>/api/host/enter?t=…`, трей, `DesktopHost` (выбор папки).
  `update.rs` — автообновление (tauri-plugin-updater, `latest.json` выпуска, подпись — секрет
  `TAURI_SIGNING_PRIVATE_KEY`, открытый ключ — в tauri.conf.json; без секрета `release.yml`
  собирает с `no-updater.conf.json`). Перед установкой — `stop_server`. Из окна: «Управление →
  Версия» (`admin/Version.svelte`) → `POST /api/admin/update/check|install` → `Host::update_*` →
  `update::from_page` (без системных окон, ход — `UpdateInfo` в `/api/admin/status`).
- `access` — `cloudpub` (рекомендуемый, `cloudpub.rs`: клиент `clo` по SHA-256, `register` один
  раз → постоянный адрес, `run`; вход почтой/паролем или ключом API, пароль не хранится),
  `fxtunnel`, `custom`, `lan`.

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
- Esc: `+layout.svelte` всегда `preventDefault` (иначе окно на Mac выходит из полноэкранного
  режима) и сам закрывает верхний `<dialog>`; в плеере Esc — меню, иначе назад к фильму.
- Логотип — `web/static/logo.svg` (белый знак, вектор с фото владельца); значки —
  `node scripts/icons.mjs`, затем `npm run icons` в `desktop/`.
- Имена файлов не должны совпадать без учёта регистра (`x.svelte.ts` и `X.svelte` — тоже):
  на Mac и Windows импорт находит не тот файл (сборка 0.1.0 так упала). Ловит
  `web/src/lib/paths.test.ts`.
- Выпуск собирается сам: `release.yml` на пуш в `miaro` смотрит `version` в `tauri.conf.json`;
  тега `v<версия>` нет — создаёт выпуск с тегом на этом коммите и собирает установщики (поднять
  версию = выпустить). Вручную — тег `vX.Y.Z`, равный `version` (иначе останавливается сразу).
  Из облачной сессии Claude тег не отправить (прокси пускает только свою ветку) — поэтому так.
- В `tauri-action` нет `tauriScript`: с `npm run tauri --` он сам добавлял ещё один `--`,
  `--target` уходил в cargo, а упаковщик искал программу в `target/release` (0.1.1 так упал).
- Mac: бандл подписывается ad-hoc (`bundle.macOS.signingIdentity: "-"`), ffmpeg — `codesign -s -`
  в `fetch-ffmpeg.sh`; без подписи Apple Silicon пишет «приложение повреждено» (0.3.0).
- hls.js — только `hls.js/light` и только лениво из плеера (это ~120 КБ gzip).
- Внешних запросов из фронта нет (шрифты системные, иконки свои); картинки IMDb/TMDB сервер
  скачивает к себе (`images/<sha256>.ext`).
- Коммиты — Conventional Commits (`feat:`, `fix:`, …).
- Ветка `miaro` — основная; владелец поручил Claude вести её самому: работа — в своей ветке →
  PR в `miaro` → зелёная CI (server, web, e2e, desktop) → Claude сливает PR сам (merge commit).
  Прямо в `miaro` не пушить. Выпуски (с 0.5) Claude собирает сам: новая `version` в PR →
  после слияния `release.yml` выпускает её сам; проследить сборку до опубликованного выпуска.

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
- Навигация: ≥1000 px — капсула в шапке (Главная, Фильмы, Сериалы, Обзор, Списки, Люди, Чаты,
  поиск), уже — нижняя панель (Главная, Обзор, Списки, Люди, Чаты; `--tabbar` — отступ под неё),
  поиск и аватар — в шапке, и капсула вкладок поверх героя главной.
- Дизайн (0.6): шрифт — только Inter (`static/fonts`, без курсива), без `backdrop-filter`
  (грузил видеокарту хоста), пустые экраны — `.empty` по центру.
- Скорость (0.6.1): без View Transitions при переходах (`onNavigate` держал старый снимок, пока
  грузится новая страница — «всё тормозит»), без `filter: blur` и бегущих градиентов
  (заглушки — пульс прозрачности), без анимации `box-shadow`. Большие кадры — `titles.backdrop_md`
  (1920 px, миграция 7, `jobs::backdrops_md`), в `Card.backdrop` — он. Сетка коллекции — по 48.
- Главная (0.6.1): герой на весь экран, `--row` (в `.home`) — высота первого ряда, он лежит на
  кадре (`.rows` с отрицательным отступом); `--hero-tint` на `<html>` красит капсулу разделов.
- Обзор (0.6.1): страница `/ext/[id]` сама ищет фильм на Rutube/VK (`/api/links/search?min=` — от 40 мин,
  у сериала от 15, название совпадает); нашёлся или есть ссылки — «Хочу на Nova» не показываем.
- Движение (0.6.2): `.page`/`.home` — `page-in`, `main.leaving` — гаснет 120 мс в `onNavigate`
  (+layout), `.rise` + `--i` — лесенка рядов и карточек (`Shelf`/`PosterRow`/`Wide`/`Poster` с `i`),
  картинки — `use:fadeIn` (`ui/fade.ts`), `Modal` закрывается анимацией (`closing`, Esc — через
  `cancel`). С 0.6.3 системное `prefers-reduced-motion` **не** учитываем (на Windows с выключенным
  «Показывать анимацию» в Nova не двигалось ничего, владелец просил анимации): выключатель —
  `lib/motion.ts` (localStorage `nova-motion`, класс `no-motion` на `<html>`, «Профиль → Анимации»).
- Вход (0.6.2): `AuthShell` — стена постеров из `/api/site/wall` (без входа, только постеры
  подборок IMDb; картинка — `/api/site/wall/{name}`, чужие имена — 404).
- Вкладки «Фильмы»/«Сериалы» (0.6.2): «Эксклюзив на Nova» (своё), «Буду смотреть» (`/api/me/marks`),
  ряды IMDb (`/api/discover`, ключи `*_movies`/`*_series`), сетка «Эксклюзив — все …»
  (`region «Вся коллекция»` — для e2e).
- Профиль (0.6.2, миграция 8): любимые фильмы — до 5 (`profile::MAX_FAVORITES`, `PUT
  /api/me/favorites {key, value}`, ключи в `/api/me` → `session.me.favorites`, фронт —
  `social/favorites.ts`), режиссёры — `profiles.directors` (до 5, подсказки — `stats.directors`),
  обложка `profiles.cover_kind`: `ribbon` — `social/PosterWall` (постеры просмотренного плывут
  вверх на фоне всей страницы, `page`; `.narrow` — свой слой и появление без transform), `image` — своя. Убрать оценку — `DELETE /api/titles/{id}/stars`.
- e2e, которые меняют дружбу или статусы, заводят своего человека (`stranger()`) или
  сбрасывают состояние (`resetStatuses`): проекты phone/tablet/desktop идут подряд на одних данных.
- Страница фильма из IMDb (0.6.4, миграция 9): `discover::detail` один раз (`ext_titles.detailed` =
  `DETAILED`) берёт описание, `moreLikeThisTitles` (похожие) и качает к себе большой постер, кадр
  (`ext_titles.backdrop`), фото актёров и кадры — всё в JSON `ext_titles.extra` (имена картинок).
  Раскладка — как у `title/[id]` (стили скопированы: общий глобальный CSS протекал бы в
  `Reviews`, `LinkCard` и окна). Обе страницы — до 1440 px шириной.
- Инструменты (0.6.4): SvelteKit 3 + Vite 8 + Vitest 5 + TypeScript 6. Настройки Kit — в
  `vite.config.ts` (`sveltekit({...})`, `svelte.config.js` больше нет); `$lib` — свой `alias`
  (в Kit 3 по умолчанию только `#lib`); `goto`: `replace: true` вместо `replaceState`, `reset: false`
  вместо `noScroll`/`keepFocus`; `page.url.searchParams` — только чтение. CI: Node 24, actions v7;
  `tauri-action` — v0 (в v1 не проверены имена файлов выпуска, по ним ищет `release.rs`).
  Сервер: reqwest 0.13 (`rustls`, `query`), rusqlite 0.41 (без `usize` в SQL), argon2 0.6 (старые
  хеши подходят — тест `passwords`), rand 0.10 (`rand::fill`), zip 9. `cargo audit`: остаётся `zip` 4
  внутри `tauri-plugin-updater` (распаковывает только подписанное нашим ключом) и
  неподдерживаемые крейты Linux-сборки Tauri.
- Слежение за папками: после установки слежения — один скан (за 5 с до него в новую папку могли
  что-то положить); чтения (`Access`, кроме «дописан») — не повод сканировать.
- База от более новой версии (0.6.5): `db::OLDEST` — сколько миграций должна знать версия, чтобы
  работать с этой базой (пишется в `settings.schema.oldest`); знает меньше — ошибка «данные от
  более новой версии Nova», приложение предлагает скачать последнюю. Новая миграция, ломающая
  старые версии, поднимает `OLDEST`. `user_version` — всегда настоящий номер (не занижать: версия
  между ними повторила бы миграции). Автозапуск переписывается на текущую копию при каждом запуске
  (после переименования в Nova он запускал старую из «Films and Serials For You»).
  Без `schema.oldest` (база от своей сборки, у владельца была 1.0.0-beta.19 с 11+ миграциями) —
  `missing_schema` сверяет с нашей схемой (миграции в памяти), `add_missing` добавляет недостающие
  таблицы, столбцы и индексы; всё есть — открываем как есть.

