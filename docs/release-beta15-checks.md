# Nova 1.0.0-beta.15

24 октября 2026. Локальная сборка Windows x64; установленное приложение и рабочая база не изменялись.

## Изменения

- Трейлер в шапке: официальный IMDb в HD (`GET /api/discover/title/{id}/trailer/hd` - mp4 1080p/720p
  через Nova, ссылка IMDb держится 6 часов, Range, только `*.media-imdb.com`). Звук - русский трейлер,
  если есть (`TrailerBanner.toggleSound`). Русские трейлеры с Rutube часто пережаты при подписи «HD».
- «Не тот фильм»: миграция 19 `bad_videos` (адреса, отмеченные неверными), `POST /api/links/{id}/wrong`
  (в чёрный список и удалить ссылку у фильма; может любой зритель), поиск `/api/links/search` их
  отфильтровывает. Плеер сравнивает длину видео с фильмом (`expect`) и при расхождении больше 12%
  показывает плашку; пункт есть и в настройках плеера. Автопоиск отбрасывает такие видео сразу.
- Каталог: `DELETE /api/admin/titles/{id}` (только без файлов) и `DELETE /api/admin/titles/empty`;
  в «Управление → Каталог» - крестик и «Убрать все без файлов».

## Проверено

- fmt, clippy `-D warnings`, `cargo test -p fsfy-server`; Svelte check 0/0; Vitest 18; build.
- Playwright phone/tablet/desktop/iphone-webkit: 313 passed, 30 skipped, 1 failed (совместный список на
  WebKit) - при повторе 3 из 3 прошли, случайное.

## Границы

Автоматическое наполнение каталога со страниц неофициальных кинотеатров не делается. Java preview
знает 18 миграций - с базой beta.15 (19) его не запускать.

Установщик: `target/release/bundle/nsis/Nova_1.0.0-beta.15_x64-setup.exe`, 62 363 632 байта, ProductVersion
`1.0.0-beta.15`. SHA-256: `7a9597ac54ad5d358a085442c1a9ace179f5edeb5056939cc6aceca772ee6c0a` (рядом `.sha256`).
Копия на рабочем столе совпадает.
