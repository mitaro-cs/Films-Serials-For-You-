//! Вход в два шага: одноразовые коды из приложения (Google Authenticator, Яндекс Ключ и т. п.) по
//! RFC 6238 - HMAC-SHA1, 6 цифр, шаг 30 с. Приложения надёжно понимают только SHA-1, а своего
//! крейта SHA-1 у нас нет - он короткий, сверен с примерами из RFC 3174/2202/6238.

/// SHA-1 (RFC 3174). Для HMAC кодов входа, не для паролей.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, x) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

/// HMAC-SHA1 (RFC 2104).
fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..20].copy_from_slice(&sha1(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let inner: Vec<u8> = k.iter().map(|b| b ^ 0x36).chain(msg.iter().copied()).collect();
    let outer: Vec<u8> = k.iter().map(|b| b ^ 0x5c).chain(sha1(&inner)).collect();
    sha1(&outer)
}

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Base32 без `=` - так ключ показывают приложения.
pub fn base32(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(5) {
        let mut buf = [0u8; 5];
        buf[..chunk.len()].copy_from_slice(chunk);
        let n = u64::from_be_bytes([0, 0, 0, buf[0], buf[1], buf[2], buf[3], buf[4]]);
        let chars = (chunk.len() * 8).div_ceil(5);
        for i in 0..chars {
            out.push(B32[((n >> (35 - i * 5)) & 31) as usize] as char);
        }
    }
    out
}

pub fn unbase32(s: &str) -> Option<Vec<u8>> {
    let mut bits = 0u64;
    let mut n = 0;
    let mut out = vec![];
    for c in s.chars().filter(|c| !c.is_whitespace() && *c != '=') {
        let v = B32.iter().position(|b| *b as char == c.to_ascii_uppercase())? as u64;
        bits = (bits << 5) | v;
        n += 5;
        if n >= 8 {
            n -= 8;
            out.push((bits >> n) as u8);
            bits &= (1 << n) - 1;
        }
    }
    Some(out)
}

/// Код на шаг времени `counter` (RFC 4226).
fn hotp(key: &[u8], counter: u64) -> u32 {
    let mac = hmac_sha1(key, &counter.to_be_bytes());
    let off = (mac[19] & 0x0f) as usize;
    let bin = u32::from_be_bytes([mac[off] & 0x7f, mac[off + 1], mac[off + 2], mac[off + 3]]);
    bin % 1_000_000
}

/// Новый ключ: 20 случайных байт в base32.
pub fn new_secret() -> String {
    let mut key = [0u8; 20];
    rand::fill(&mut key);
    base32(&key)
}

/// Код верный для времени `now_ms` (±30 с - часы телефона могут отставать).
pub fn check(secret: &str, code: &str, now_ms: i64) -> bool {
    let code: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    let (Some(key), Ok(want)) = (unbase32(secret), code.parse::<u32>()) else { return false };
    if code.len() != 6 || key.is_empty() {
        return false;
    }
    let step = (now_ms / 1000 / 30) as u64;
    [step.wrapping_sub(1), step, step + 1].iter().any(|s| hotp(&key, *s) == want)
}

/// Код на момент `now_ms` - как его покажет приложение (для проверок).
pub fn code(secret: &str, now_ms: i64) -> Option<String> {
    let key = unbase32(secret)?;
    Some(format!("{:06}", hotp(&key, (now_ms / 1000 / 30) as u64)))
}

/// Ссылка для приложения: `otpauth://totp/Nova:логин?secret=…&issuer=Nova`.
pub fn uri(secret: &str, login: &str) -> String {
    let label: String = url::form_urlencoded::byte_serialize(format!("Nova:{login}").as_bytes()).collect();
    format!("otpauth://totp/{label}?secret={secret}&issuer=Nova&digits=6&period=30")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_and_hmac_match_rfc() {
        assert_eq!(hex::encode(sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            hex::encode(sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        // RFC 2202, тест 2.
        assert_eq!(
            hex::encode(hmac_sha1(b"Jefe", b"what do ya want for nothing?")),
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
        );
    }

    #[test]
    fn totp_matches_rfc6238() {
        // RFC 6238, приложение B: ключ «12345678901234567890», SHA-1, 8 цифр → у нас последние 6.
        let key = base32(b"12345678901234567890");
        assert_eq!(key, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert_eq!(unbase32(&key).unwrap(), b"12345678901234567890");
        for (t, code) in [(59, "287082"), (1111111109, "081804"), (1234567890, "005924"), (2000000000, "279037")] {
            assert!(check(&key, code, t * 1000), "{t}");
        }
        assert!(!check(&key, "000000", 59_000));
        assert!(!check(&key, "28708", 59_000));
        assert!(uri(&key, "host").starts_with("otpauth://totp/Nova%3Ahost?secret=GEZ"));
    }
}
