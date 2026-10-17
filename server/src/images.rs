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

/// Цвет карточки по постеру (как низ билета на макете): преобладающий оттенок картинки,
/// приглушённый и тёмный — белый текст на нём всегда читается. `rgb` — пиксели RGB подряд.
pub fn accent(rgb: &[u8]) -> String {
    const BUCKETS: usize = 12;
    let mut sum = [[0.0f64; 4]; BUCKETS];
    let mut grey = [0.0f64; 4];
    for px in rgb.chunks_exact(3) {
        let (r, g, b) = (px[0] as f64 / 255.0, px[1] as f64 / 255.0, px[2] as f64 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let s = if max > 0.0 { (max - min) / max } else { 0.0 };
        // Слишком тёмное и пересвеченное почти не голосует.
        let lit = if (0.18..=0.95).contains(&max) { 1.0 } else { 0.15 };
        if s < 0.15 {
            for (i, c) in [r, g, b].into_iter().enumerate() {
                grey[i] += c * lit;
            }
            grey[3] += lit;
            continue;
        }
        let h = hue(r, g, b, max, min);
        let w = s * lit;
        let k = ((h / 360.0 * BUCKETS as f64) as usize).min(BUCKETS - 1);
        for (i, c) in [r, g, b].into_iter().enumerate() {
            sum[k][i] += c * w;
        }
        sum[k][3] += w;
    }
    let best = sum.iter().max_by(|a, b| a[3].total_cmp(&b[3])).copied().unwrap_or_default();
    let (r, g, b, colorful) = if best[3] > grey[3] * 0.08 && best[3] > 0.0 {
        (best[0] / best[3], best[1] / best[3], best[2] / best[3], true)
    } else if grey[3] > 0.0 {
        (grey[0] / grey[3], grey[1] / grey[3], grey[2] / grey[3], false)
    } else {
        (0.2, 0.2, 0.22, false)
    };
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let h = hue(r, g, b, max, min);
    let l = (max + min) / 2.0;
    let s = if max == min { 0.0 } else { (max - min) / (1.0 - (2.0 * l - 1.0).abs()) };
    let s = if colorful { s.clamp(0.18, 0.38) } else { s.min(0.06) };
    let (r, g, b) = hsl(h, s, 0.3);
    format!("#{:02x}{:02x}{:02x}", (r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
}

fn hue(r: f64, g: f64, b: f64, max: f64, min: f64) -> f64 {
    let d = max - min;
    if d <= 0.0 {
        return 0.0;
    }
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h * 60.0
}

fn hsl(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
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

    fn contrast_with_white(hex: &str) -> f64 {
        let ch = |i: usize| {
            let c = u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f64 / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let l = 0.2126 * ch(1) + 0.7152 * ch(3) + 0.0722 * ch(5);
        1.05 / (l + 0.05)
    }

    #[test]
    fn accent_follows_poster_and_keeps_text_readable() {
        let fill = |r: u8, g: u8, b: u8| [r, g, b].repeat(200);
        // Красный постер — красноватый низ, серый — серый.
        let red = accent(&fill(200, 40, 30));
        assert!(u8::from_str_radix(&red[1..3], 16).unwrap() > u8::from_str_radix(&red[3..5], 16).unwrap());
        let grey = accent(&fill(128, 128, 128));
        assert_eq!(&grey[1..3], &grey[5..7]);
        // Мелкое яркое пятно на тёмном фоне всё равно задаёт оттенок.
        let mut mixed = fill(10, 10, 12);
        mixed.extend(fill(30, 120, 220).iter().take(90));
        let blue = accent(&mixed);
        assert!(u8::from_str_radix(&blue[5..7], 16).unwrap() > u8::from_str_radix(&blue[1..3], 16).unwrap());
        // Любой оттенок: белый текст читается (WCAG AA, 4.5:1).
        for h in (0..360).step_by(15) {
            let (r, g, b) = hsl(h as f64, 1.0, 0.5);
            let c = accent(&fill((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8));
            assert!(contrast_with_white(&c) >= 4.5, "{c} для {h}°");
        }
        assert!(accent(&[]).starts_with('#'));
    }
}
