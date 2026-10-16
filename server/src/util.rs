//! Мелочи, нужные везде: время, случайные токены, хеши, нормализация названий.

use rand::RngCore;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// Миллисекунды Unix (UTC) — так время хранится в базе и отдаётся в API.
pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Случайная строка base64url из `bytes` байт.
pub fn token(bytes: usize) -> String {
    use base64::Engine;
    let mut b = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut b);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// Код без похожих символов (0/O, 1/I/l) — для приглашений.
pub fn human_code(len: usize) -> String {
    const ABC: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut b = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut b);
    b.iter().map(|x| ABC[*x as usize % ABC.len()] as char).collect()
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Ключ для сравнения названий: строчные, «ё» → «е», только буквы и цифры через пробел.
pub fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars().flat_map(char::to_lowercase) {
        let c = if c == 'ё' { 'е' } else { c };
        if c.is_alphanumeric() {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        } else {
            space = true;
        }
    }
    out
}

/// Безопасное имя для заголовка Content-Disposition (RFC 6266 + RFC 5987).
pub fn content_disposition(kind: &str, name: &str) -> String {
    let ascii: String =
        name.chars().map(|c| if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' { c } else { '_' }).collect();
    let mut enc = String::new();
    for b in name.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(b) {
            enc.push(*b as char);
        } else {
            enc.push_str(&format!("%{b:02X}"));
        }
    }
    format!("{kind}; filename=\"{ascii}\"; filename*=UTF-8''{enc}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_folds_case_and_yo() {
        assert_eq!(norm("Ёлки-Палки: 2!"), "елки палки 2");
        assert_eq!(norm("  The.Office  "), "the office");
    }

    #[test]
    fn disposition_is_ascii_safe() {
        let h = content_disposition("attachment", "Фильм \"1\".mkv");
        assert!(h.is_ascii());
        assert!(h.contains("filename*=UTF-8''%D0%A4"));
    }
}
