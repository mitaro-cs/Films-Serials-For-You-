//! Картинки (постеры, фоны, кадры): файлы `<sha256>.<ext>` в каталоге `images`.
//! Имя — хеш содержимого, поэтому файлы можно кешировать навсегда.

use anyhow::Context;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone)]
pub struct Images {
    dir: PathBuf,
    http: reqwest::Client,
}

pub const MAX_IMAGE: usize = 15 * 1024 * 1024;

/// Тип картинки по первым байтам; всё, что не JPEG/PNG/WebP, не принимаем.
pub fn sniff(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

pub fn valid_name(name: &str) -> bool {
    let (hash, ext) = match name.split_once('.') {
        Some(x) => x,
        None => return false,
    };
    hash.len() == 64
        && hash.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && matches!(ext, "jpg" | "png" | "webp")
}

impl Images {
    pub fn new(dir: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("fsfy/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { dir, http })
    }

    pub fn path(&self, name: &str) -> Option<PathBuf> {
        valid_name(name).then(|| self.dir.join(name))
    }

    pub fn save(&self, bytes: &[u8]) -> anyhow::Result<String> {
        let ext = sniff(bytes).context("это не картинка (нужен JPEG, PNG или WebP)")?;
        let name = format!("{}.{ext}", crate::util::sha256_hex(bytes));
        let path = self.dir.join(&name);
        if !path.exists() {
            let part = self.dir.join(format!("{name}.part"));
            std::fs::write(&part, bytes)?;
            std::fs::rename(&part, &path)?;
        }
        Ok(name)
    }

    pub fn import(&self, file: &Path) -> anyhow::Result<String> {
        let meta = std::fs::metadata(file)?;
        anyhow::ensure!(meta.len() as usize <= MAX_IMAGE, "картинка больше 15 МБ");
        self.save(&std::fs::read(file)?)
    }

    pub async fn download(&self, url: &str) -> anyhow::Result<String> {
        let res = self.http.get(url).send().await?.error_for_status()?;
        if res.content_length().is_some_and(|l| l as usize > MAX_IMAGE) {
            anyhow::bail!("картинка больше 15 МБ");
        }
        let bytes = res.bytes().await?;
        anyhow::ensure!(bytes.len() <= MAX_IMAGE, "картинка больше 15 МБ");
        let me = self.clone();
        tokio::task::spawn_blocking(move || me.save(&bytes)).await?
    }

    /// Картинки рядом с видео: poster.jpg, folder.jpg, cover.jpg / fanart.jpg, backdrop.jpg.
    pub fn local_art(dir: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
        let find = |names: &[&str]| {
            names
                .iter()
                .flat_map(|n| ["jpg", "jpeg", "png", "webp"].map(|e| dir.join(format!("{n}.{e}"))))
                .find(|p| p.is_file())
        };
        (find(&["poster", "folder", "cover"]), find(&["fanart", "backdrop", "background"]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_by_hash_and_rejects_non_images() {
        let dir = tempfile::tempdir().unwrap();
        let im = Images::new(dir.path().to_path_buf()).unwrap();
        let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];
        let name = im.save(&jpg).unwrap();
        assert!(valid_name(&name) && name.ends_with(".jpg"));
        assert!(im.path(&name).unwrap().is_file());
        assert!(im.save(b"<svg/>").is_err());
        assert!(im.path("../../etc/passwd").is_none());
    }
}
