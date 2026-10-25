//! Архив ZIP потоком - «скачать весь сериал / сезон / фильм одним файлом». Видео не сжимается,
//! поэтому файлы кладутся как есть (Stored), CRC считается на лету, размеры известны заранее -
//! браузер видит точный размер архива (`total`) и показывает, сколько осталось. Всегда ZIP64:
//! сериалы бывают больше 4 ГБ. Временных файлов нет - архив собирается, пока его скачивают.

use std::io::{Read, Write};
use std::path::PathBuf;

pub struct Entry {
    /// Имя внутри архива («Сезон 1/Тьма.s01e01.mkv»).
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
}

const LOCAL: u64 = 30 + 20;
const DESCRIPTOR: u64 = 24;
const CENTRAL: u64 = 46 + 28;
const TAIL: u64 = 56 + 20 + 22;

/// Точный размер архива - для Content-Length.
pub fn total(entries: &[Entry]) -> u64 {
    entries.iter().map(|e| LOCAL + DESCRIPTOR + CENTRAL + 2 * e.name.len() as u64 + e.size).sum::<u64>() + TAIL
}

/// Время в формате DOS - фиксированное (1 января 2020): одинаковый архив при каждом скачивании.
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = ((2020 - 1980) << 9) | (1 << 5) | 1;
/// Бит 3 - CRC после данных, бит 11 - имена в UTF-8 (кириллица).
const FLAGS: u16 = 0x0808;

fn u16le(w: &mut Vec<u8>, v: u16) {
    w.extend_from_slice(&v.to_le_bytes());
}
fn u32le(w: &mut Vec<u8>, v: u32) {
    w.extend_from_slice(&v.to_le_bytes());
}
fn u64le(w: &mut Vec<u8>, v: u64) {
    w.extend_from_slice(&v.to_le_bytes());
}

fn local_header(name: &str, size: u64) -> Vec<u8> {
    let mut h = Vec::with_capacity(LOCAL as usize + name.len());
    u32le(&mut h, 0x0403_4b50);
    u16le(&mut h, 45);
    u16le(&mut h, FLAGS);
    u16le(&mut h, 0);
    u16le(&mut h, DOS_TIME);
    u16le(&mut h, DOS_DATE);
    u32le(&mut h, 0);
    u32le(&mut h, u32::MAX);
    u32le(&mut h, u32::MAX);
    u16le(&mut h, name.len() as u16);
    u16le(&mut h, 20);
    h.extend_from_slice(name.as_bytes());
    u16le(&mut h, 0x0001);
    u16le(&mut h, 16);
    u64le(&mut h, size);
    u64le(&mut h, size);
    h
}

fn descriptor(crc: u32, size: u64) -> Vec<u8> {
    let mut h = Vec::with_capacity(DESCRIPTOR as usize);
    u32le(&mut h, 0x0807_4b50);
    u32le(&mut h, crc);
    u64le(&mut h, size);
    u64le(&mut h, size);
    h
}

fn central(name: &str, crc: u32, size: u64, offset: u64) -> Vec<u8> {
    let mut h = Vec::with_capacity(CENTRAL as usize + name.len());
    u32le(&mut h, 0x0201_4b50);
    u16le(&mut h, 45);
    u16le(&mut h, 45);
    u16le(&mut h, FLAGS);
    u16le(&mut h, 0);
    u16le(&mut h, DOS_TIME);
    u16le(&mut h, DOS_DATE);
    u32le(&mut h, crc);
    u32le(&mut h, u32::MAX);
    u32le(&mut h, u32::MAX);
    u16le(&mut h, name.len() as u16);
    u16le(&mut h, 28);
    u16le(&mut h, 0);
    u16le(&mut h, 0);
    u16le(&mut h, 0);
    u32le(&mut h, 0);
    u32le(&mut h, u32::MAX);
    h.extend_from_slice(name.as_bytes());
    u16le(&mut h, 0x0001);
    u16le(&mut h, 24);
    u64le(&mut h, size);
    u64le(&mut h, size);
    u64le(&mut h, offset);
    h
}

fn tail(count: u64, cd_size: u64, cd_offset: u64) -> Vec<u8> {
    let mut h = Vec::with_capacity(TAIL as usize);
    let z64 = cd_offset + cd_size;
    u32le(&mut h, 0x0606_4b50);
    u64le(&mut h, 44);
    u16le(&mut h, 45);
    u16le(&mut h, 45);
    u32le(&mut h, 0);
    u32le(&mut h, 0);
    u64le(&mut h, count);
    u64le(&mut h, count);
    u64le(&mut h, cd_size);
    u64le(&mut h, cd_offset);
    u32le(&mut h, 0x0706_4b50);
    u32le(&mut h, 0);
    u64le(&mut h, z64);
    u32le(&mut h, 1);
    u32le(&mut h, 0x0605_4b50);
    u16le(&mut h, 0);
    u16le(&mut h, 0);
    u16le(&mut h, count.min(0xFFFF) as u16);
    u16le(&mut h, count.min(0xFFFF) as u16);
    u32le(&mut h, u32::MAX);
    u32le(&mut h, u32::MAX);
    u16le(&mut h, 0);
    h
}

/// Записать архив в `out`. Файл стал другого размера, пока качали, - ошибка (архив был бы битым).
pub fn write(entries: &[Entry], out: &mut impl Write) -> std::io::Result<()> {
    let mut offset = 0u64;
    let mut dir = Vec::new();
    let mut buf = vec![0u8; 1 << 20];
    for e in entries {
        let h = local_header(&e.name, e.size);
        out.write_all(&h)?;
        let mut f = std::fs::File::open(&e.path)?;
        let mut crc = flate2::Crc::new();
        let mut done = 0u64;
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let n = n.min((e.size - done) as usize);
            crc.update(&buf[..n]);
            out.write_all(&buf[..n])?;
            done += n as u64;
            if done == e.size {
                break;
            }
        }
        if done != e.size {
            return Err(std::io::Error::other(format!("{} изменился во время скачивания", e.name)));
        }
        out.write_all(&descriptor(crc.sum(), e.size))?;
        dir.extend(central(&e.name, crc.sum(), e.size, offset));
        offset += h.len() as u64 + e.size + DESCRIPTOR;
    }
    out.write_all(&dir)?;
    out.write_all(&tail(entries.len() as u64, dir.len() as u64, offset))?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_opens_and_size_is_exact() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.mkv");
        let b = dir.path().join("b.mp4");
        std::fs::write(&a, vec![7u8; 3_000_000]).unwrap();
        std::fs::write(&b, b"hello").unwrap();
        let entries = vec![
            Entry { name: "Сезон 1/Тьма 1×01.mkv".into(), path: a.clone(), size: 3_000_000 },
            Entry { name: "Сезон 1/b.mp4".into(), path: b.clone(), size: 5 },
        ];
        let mut out = Vec::new();
        write(&entries, &mut out).unwrap();
        assert_eq!(out.len() as u64, total(&entries));
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(out)).unwrap();
        assert_eq!(z.len(), 2);
        let mut s = String::new();
        z.by_name("Сезон 1/b.mp4").unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "hello");
        let mut big = Vec::new();
        z.by_name("Сезон 1/Тьма 1×01.mkv").unwrap().read_to_end(&mut big).unwrap();
        assert_eq!(big.len(), 3_000_000);
    }
}
