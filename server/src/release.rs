//! Последний выпуск на GitHub - для обновления приложения хоста, когда в выпуске нет файлов
//! автообновления Tauri (`latest.json` подписывается секретом; без него - эта дорога): узнать
//! версию, выбрать установщик под систему, скачать его. Ставит приложение (`desktop/update.rs`).

use futures_util::StreamExt;
use serde::Deserialize;
use std::path::Path;
use tokio::io::AsyncWriteExt;

pub const REPO: &str = "mitaro-cs/Films-Serials-For-You-";

#[derive(Debug, Clone)]
pub struct Latest {
    pub version: String,
    /// Установщик для этой системы: имя и адрес.
    pub asset: Option<(String, String)>,
    pub page: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// 1.10.0 > 1.9.3; хвост вроде `-beta.1` - младше выпуска.
pub fn newer(candidate: &str, current: &str) -> bool {
    fn parts(v: &str) -> (Vec<u64>, bool) {
        let v = v.trim().trim_start_matches('v');
        let (main, pre) = v.split_once('-').map_or((v, false), |(m, _)| (m, true));
        (main.split('.').map(|p| p.parse().unwrap_or(0)).collect(), pre)
    }
    let (a, ap) = parts(candidate);
    let (b, bp) = parts(current);
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    !ap && bp
}

/// Установщик под систему: Windows - `*-setup.exe`, Mac - `.dmg` своей архитектуры.
pub fn pick(names: &[String], os: &str, arch: &str) -> Option<usize> {
    let mac_arch = if arch == "aarch64" { "aarch64" } else { "x64" };
    names.iter().position(|n| match os {
        "windows" => n.ends_with("-setup.exe"),
        "macos" => n.ends_with(&format!("_{mac_arch}.dmg")),
        _ => false,
    })
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("fsfy/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

pub async fn latest(http: &reqwest::Client) -> anyhow::Result<Latest> {
    let r: Release = http
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    anyhow::ensure!(!r.draft && !r.prerelease, "нет выпуска");
    let names: Vec<String> = r.assets.iter().map(|a| a.name.clone()).collect();
    let asset = pick(&names, std::env::consts::OS, std::env::consts::ARCH)
        .map(|i| (r.assets[i].name.clone(), r.assets[i].browser_download_url.clone()));
    Ok(Latest { version: r.tag_name.trim_start_matches('v').to_string(), asset, page: r.html_url })
}

/// Скачать в файл (сначала `*.part`), `progress(получено, всего)`.
pub async fn download(
    http: &reqwest::Client,
    url: &str,
    to: &Path,
    mut progress: impl FnMut(u64, Option<u64>),
) -> anyhow::Result<()> {
    let r = http.get(url).timeout(std::time::Duration::from_secs(3600)).send().await?.error_for_status()?;
    let total = r.content_length();
    let part = to.with_extension("part");
    let mut f = tokio::fs::File::create(&part).await?;
    let mut got = 0u64;
    let mut body = r.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        got += chunk.len() as u64;
        f.write_all(&chunk).await?;
        progress(got, total);
    }
    f.sync_all().await?;
    drop(f);
    tokio::fs::rename(&part, to).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_assets() {
        assert!(newer("0.6.0", "0.5.0"));
        assert!(newer("v1.10.0", "1.9.3"));
        assert!(!newer("0.5.0", "0.5.0"));
        assert!(!newer("0.5.0-beta.1", "0.5.0"));
        assert!(newer("0.5.0", "0.5.0-beta.1"));
        let names: Vec<String> =
            ["Nova_0.6.0_aarch64.dmg", "Nova_0.6.0_x64.dmg", "Nova_0.6.0_x64-setup.exe", "Nova_aarch64.app.tar.gz"]
                .map(String::from)
                .to_vec();
        assert_eq!(pick(&names, "macos", "aarch64"), Some(0));
        assert_eq!(pick(&names, "macos", "x86_64"), Some(1));
        assert_eq!(pick(&names, "windows", "x86_64"), Some(2));
        assert_eq!(pick(&names, "linux", "x86_64"), None);
    }
}
