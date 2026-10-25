//! Клиент CloudPub (cloudpub.ru, открытый клиент - github.com/ermak-dev/cloudpub): российский
//! туннель, связь с сервером - WebSocket по TLS на порту 443. Поэтому работает и с VPN, и без, и в
//! сетях, где открыт только обычный HTTPS.
//!
//! Адрес постоянный: сервис один раз регистрируется (`clo register`), а при каждом запуске
//! поднимается командой `clo run` с тем же адресом. Настройки клиента (в том числе его токен) -
//! в `tools/cloudpub/client.toml` каталога данных; пароль нигде не хранится.

use regex::Regex;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;

pub const VERSION: &str = "3.6.902";
const BUILDS: &[(&str, &str, &str)] = &[
    ("darwin-arm64", "macos-aarch64.tar.gz", "b1def4e0d9e13eb638bead56ad069f825e662f0b238fd4fde2e6f7e122ddef27"),
    ("darwin-amd64", "macos-x86_64.tar.gz", "3aef7d90b9ae7f3a1ee3f5d862372f18a544dc7a164b3de4b3a0d6ebcb4b4063"),
    ("windows-amd64", "windows-x86_64.zip", "1a9bcd9487a678c0cf6bb61dcf7e998c1dfcc142f1b6c0ab31efb78c208e3ae2"),
    ("linux-amd64", "linux-x86_64.tar.gz", "a77711855f39d197a361f44d7d1aec6642d78064789c9d50fc34079dcaef9969"),
    ("linux-arm64", "linux-aarch64.tar.gz", "4b8cb10f8e7de7379812555b8a0083c1375bd94f20133f08db23df05c828c88a"),
];

/// «… -> https://имя.cloudpub.ru:443/» - адрес сервиса в строке клиента.
pub static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"->\s*(https://[a-z0-9.-]+\.cloudpub\.ru)").unwrap());
static TOKEN_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?m)^\s*token\s*=\s*"[^"]+""#).unwrap());
pub static AUTH_ERROR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(unauthori[sz]ed|invalid\s+(token|credentials|password)|token.{0,12}(invalid|expired)|expired\s+token|authentication failed|authorization failed|неверн.{0,20}(токен|парол)|ошибк.{0,20}авторизац)").unwrap()
});
static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new("\u{1b}\\[[0-9;?]*[A-Za-z]").unwrap());

pub struct CloudPub {
    dir: PathBuf,
}

/// Итог короткой команды клиента.
pub struct Out {
    pub ok: bool,
    pub text: String,
}

impl Out {
    /// Последняя непустая строка - обычно в ней и объяснение ошибки.
    pub fn last_line(&self) -> String {
        self.text.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default().to_string()
    }
}

impl CloudPub {
    pub fn new(data: &Path) -> Self {
        Self { dir: data.join("tools/cloudpub") }
    }

    pub fn conf(&self) -> PathBuf {
        self.dir.join("client.toml")
    }

    pub fn pid_file(&self) -> PathBuf {
        self.dir.join("run.pid")
    }

    fn exe(&self) -> PathBuf {
        self.dir.join(format!("bin-{VERSION}")).join(if cfg!(windows) { "clo.exe" } else { "clo" })
    }

    pub fn installed(&self) -> bool {
        self.exe().is_file()
    }

    /// Токен сохранён в настройках клиента - вход выполнен.
    pub fn logged_in(&self) -> bool {
        std::fs::read_to_string(self.conf()).is_ok_and(|s| TOKEN_LINE.is_match(&s))
    }

    /// Клиент на диске; если нет - скачать с cloudpub.ru и сверить SHA-256.
    pub async fn ensure(&self, http: &reqwest::Client) -> anyhow::Result<PathBuf> {
        let exe = self.exe();
        if exe.is_file() {
            return Ok(exe);
        }
        let key = super::access::platform().ok_or_else(|| anyhow::anyhow!("для этой системы нет клиента CloudPub"))?;
        let (_, asset, sha) = BUILDS
            .iter()
            .find(|(k, _, _)| *k == key)
            .ok_or_else(|| anyhow::anyhow!("для {key} нет клиента CloudPub"))?;
        let url = format!("https://cloudpub.ru/download/stable/clo-{VERSION}-stable-{asset}");
        let bytes = http.get(url).timeout(Duration::from_secs(300)).send().await?.error_for_status()?.bytes().await?;
        if hex::encode(Sha256::digest(&bytes)) != *sha {
            anyhow::bail!("скачанный клиент CloudPub не совпал с ожидаемым - не запускаем");
        }
        let name = exe.file_name().and_then(|n| n.to_str()).unwrap_or("clo").to_string();
        let bin = if asset.ends_with(".zip") { from_zip(&bytes, &name)? } else { from_tar_gz(&bytes, &name)? };
        std::fs::create_dir_all(exe.parent().expect("binary directory"))?;
        let part = exe.with_extension("part");
        std::fs::write(&part, bin)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755))?;
        }
        std::fs::rename(&part, &exe)?;
        Ok(exe)
    }

    /// Короткая команда клиента (вход, регистрация, список).
    pub async fn run(&self, args: &[&str], env: &[(&str, &str)], secs: u64) -> anyhow::Result<Out> {
        std::fs::create_dir_all(&self.dir)?;
        let mut cmd = crate::library::probe::command(&self.exe());
        cmd.arg("--conf")
            .arg(self.conf())
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = tokio::time::timeout(Duration::from_secs(secs), cmd.output())
            .await
            .map_err(|_| anyhow::anyhow!("CloudPub не ответил за {secs} с"))??;
        self.restrict();
        let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        Ok(Out { ok: out.status.success(), text: ANSI.replace_all(&text, "").into_owned() })
    }

    /// Вход почтой и паролем. Пароль - через переменную окружения, в командной строке его не видно.
    pub async fn login(&self, email: &str, password: &str) -> anyhow::Result<Out> {
        self.run(&["login", email], &[("CLO_PASSWORD", password)], 60).await
    }

    /// Вход ключом API из личного кабинета (если на cloudpub.ru входят через Яндекс или VK).
    pub async fn set_token(&self, token: &str) -> anyhow::Result<Out> {
        self.run(&["set", "token", token], &[], 30).await
    }

    pub async fn logout(&self) -> anyhow::Result<Out> {
        self.run(&["logout"], &[], 30).await
    }

    /// Адрес уже зарегистрированного сервиса для этого порта - чтобы не плодить новые адреса.
    pub async fn registered(&self, port: u16) -> anyhow::Result<Option<String>> {
        let out = self.run(&["ls"], &[], 60).await?;
        anyhow::ensure!(out.ok, "{}", out.last_line());
        Ok(find_registered(&out.text, port))
    }

    /// Регистрирует сервис и возвращает его постоянный адрес.
    pub async fn register(&self, port: u16) -> anyhow::Result<String> {
        let target = format!("127.0.0.1:{port}");
        let out = self.run(&["register", "-n", "fsfy", "http", &target], &[], 90).await?;
        match find_url(&out.text) {
            Some(u) if out.ok => Ok(u),
            _ => {
                let why = out.last_line();
                anyhow::bail!("{}", if why.is_empty() { "CloudPub не выдал адрес".to_string() } else { why })
            }
        }
    }

    /// Долгоживущая команда: поднимает все зарегистрированные сервисы.
    pub fn run_command(&self) -> tokio::process::Command {
        let mut cmd = crate::library::probe::command(&self.exe());
        cmd.arg("--conf").arg(self.conf()).arg("run");
        cmd
    }

    /// В настройках клиента лежит его токен - только для владельца.
    fn restrict(&self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if self.conf().is_file() {
                let _ = std::fs::set_permissions(self.conf(), std::fs::Permissions::from_mode(0o600));
            }
        }
    }
}

pub fn find_url(text: &str) -> Option<String> {
    URL.captures_iter(text).last().map(|c| c[1].to_string())
}

fn find_registered(text: &str, port: u16) -> Option<String> {
    let ours =
        Regex::new(&format!(r"http://127\.0\.0\.1:{port}(?:/\S*)?\s*->\s*(https://[a-z0-9.-]+\.cloudpub\.ru)")).ok()?;
    ours.captures(text).map(|c| c[1].to_string())
}

fn from_zip(bytes: &[u8], name: &str) -> anyhow::Result<Vec<u8>> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    for i in 0..z.len() {
        let mut f = z.by_index(i)?;
        if f.is_file() && f.name().rsplit('/').next() == Some(name) {
            let mut out = Vec::new();
            f.read_to_end(&mut out)?;
            return Ok(out);
        }
    }
    anyhow::bail!("в архиве CloudPub нет {name}")
}

fn from_tar_gz(bytes: &[u8], name: &str) -> anyhow::Result<Vec<u8>> {
    let mut t = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    for e in t.entries()? {
        let mut e = e?;
        let file = e.header().entry_type().is_file();
        if file && e.path()?.file_name().and_then(|n| n.to_str()) == Some(name) {
            let mut out = Vec::new();
            e.read_to_end(&mut out)?;
            return Ok(out);
        }
    }
    anyhow::bail!("в архиве CloudPub нет {name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_client_output() {
        let run = "\u{1b}[32mСервис опубликован: http://127.0.0.1:17480 -> https://kino-abc.cloudpub.ru:443/\u{1b}[0m";
        assert_eq!(find_url(&ANSI.replace_all(run, "")).as_deref(), Some("https://kino-abc.cloudpub.ru"));
        let ls = "fsfy http://127.0.0.1:9999 -> https://other.cloudpub.ru\nfsfy http://127.0.0.1:17480 -> https://kino-abc.cloudpub.ru:443/";
        assert_eq!(find_registered(ls, 17480).as_deref(), Some("https://kino-abc.cloudpub.ru"));
        assert_eq!(find_registered(ls, 1748), None, "порт целиком, а не начало");
        assert!(AUTH_ERROR.is_match("Ошибка: неверный токен"));
        assert!(!AUTH_ERROR.is_match("connection reset"));
        assert!(!AUTH_ERROR.is_match("login successful, token refreshed"));
        assert!(AUTH_ERROR.is_match("Token is expired"));
        let o = Out { ok: false, text: "a\n  причина  \n\n".into() };
        assert_eq!(o.last_line(), "причина");
    }

    #[test]
    fn extracts_binary_from_archives() {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        z.start_file("dir/clo.exe", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(b"MZbin").unwrap();
        let zip = z.finish().unwrap().into_inner();
        assert_eq!(from_zip(&zip, "clo.exe").unwrap(), b"MZbin");
        assert!(from_zip(&zip, "clo").is_err());

        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        let mut h = tar::Header::new_gnu();
        h.set_size(4);
        h.set_mode(0o755);
        h.set_cksum();
        tar.append_data(&mut h, "clo-3.5/clo", &b"ELF!"[..]).unwrap();
        let gz = tar.into_inner().unwrap().finish().unwrap();
        assert_eq!(from_tar_gz(&gz, "clo").unwrap(), b"ELF!");
    }

    #[test]
    fn login_state_from_conf() {
        let dir = tempfile::tempdir().unwrap();
        let cp = CloudPub::new(dir.path());
        assert!(!cp.logged_in());
        std::fs::create_dir_all(cp.conf().parent().unwrap()).unwrap();
        std::fs::write(cp.conf(), "server = \"x\"\ntoken = \"\"\n").unwrap();
        assert!(!cp.logged_in(), "пустой токен - не вход");
        std::fs::write(cp.conf(), "token = \"abc\"\n").unwrap();
        assert!(cp.logged_in());
    }
}
