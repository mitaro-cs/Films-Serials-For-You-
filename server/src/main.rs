//! `fsfy serve` — сервер без приложения (домашний сервер, NAS, разработка).

use std::path::PathBuf;
use std::sync::Arc;

const HELP: &str = "Nova — домашний кинотеатр

Использование:
  fsfy serve [--data КАТАЛОГ] [--port ПОРТ] [--window-port ПОРТ]

  --data         каталог данных (по умолчанию — как у приложения, или $FSFY_DATA)
  --port         порт для зрителей (по умолчанию 17480)
  --window-port  порт окна хоста, только 127.0.0.1 (по умолчанию 17481)

После запуска в консоли — ссылка входа хоста (одноразовая, на 2 минуты).";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,fsfy=info".into()),
        )
        .init();
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    if cmd != "serve" {
        println!("{HELP}");
        return Ok(());
    }
    let mut opts = fsfy::Options { data: fsfy::default_data_dir(), port: None, window_port: None };
    while let Some(a) = args.next() {
        let mut val = || args.next().ok_or_else(|| anyhow::anyhow!("после {a} нужно значение"));
        match a.as_str() {
            "--data" | "-d" => opts.data = PathBuf::from(val()?),
            "--port" => opts.port = Some(val()?.parse()?),
            "--window-port" => opts.window_port = Some(val()?.parse()?),
            _ => anyhow::bail!("неизвестный параметр {a}\n\n{HELP}"),
        }
    }
    let running = fsfy::start(opts, Arc::new(fsfy::host::Headless)).await?;
    println!("Сайт для зрителей: http://localhost:{}", running.app.ports.0);
    println!("Вход хоста:        {}", running.entry_url());
    let app = running.app.clone();
    tokio::spawn(async move {
        // Новая ссылка входа по Enter в консоли.
        let mut line = String::new();
        while std::io::stdin().read_line(&mut line).is_ok_and(|n| n > 0) {
            println!("Вход хоста: {}", app.window_entry());
            line.clear();
        }
    });
    tokio::select! {
        _ = tokio::signal::ctrl_c() => running.shutdown().await,
    }
    Ok(())
}
