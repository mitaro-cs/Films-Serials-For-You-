# Alpha 0.1 (0.1.0-alpha) - проверки выпуска

25 октября 2026, Windows x64, ветка `alpha-0.1` (коммит слияния локальной 1.0.0-beta.19 и
`origin/miaro` 0.6.6). Резерв до слияния: ветка `local/1.0-beta19`, тег `backup/local-beta19`.

| Проверка | Итог |
| --- | --- |
| `cargo fmt --all --check` | чисто |
| `cargo clippy -p fsfy-server --all-targets -- -D warnings` | чисто |
| `cargo test -p fsfy-server` (с ffmpeg) | все прошли, в том числе новый `github_base_gets_our_migrations` |
| `npm run check` (web) | 0 ошибок, 0 предупреждений |
| `npm test` (web) | 18 из 18 |
| `npm run build` (web) | собрано |
| `npm audit` (web, desktop) | 0 уязвимостей |
| `npx playwright test` (iPhone WebKit, телефон, планшет, компьютер) | 326 прошли, 26 пропущены, 0 упали |
| JS главной | 65,4 КБ сжатого (бюджет 80) |

Установщик: `Nova_0.1.0-alpha_x64-setup.exe`, 60,3 МиБ,
SHA-256 `1a7aabcf95e78905d3678710945afd47a04836418b913e1291007cfb506cbb43`.

На GitHub ничего не отправлялось.
