//! A streaming ZIP writer for photo downloads.
//!
//! Photos are already compressed, so entries are stored, not deflated: the archive is the
//! files back to back with headers between them, written as the bytes arrive from object
//! storage. Nothing is buffered beyond one chunk, so a job's 120 originals never sit in
//! memory at once. Sizes and checksums follow each entry in a data descriptor, because the
//! checksum is only known once the bytes have passed. ZIP64 records are written only where a
//! size or an offset outgrows 32 bits, so ordinary archives stay readable by anything.

const LOCAL_HEADER: u32 = 0x0403_4b50;
const DATA_DESCRIPTOR: u32 = 0x0807_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_OF_CENTRAL: u32 = 0x0605_4b50;
const ZIP64_END: u32 = 0x0606_4b50;
const ZIP64_LOCATOR: u32 = 0x0706_4b50;
/// Bit 3: sizes and CRC follow the data. Bit 11: the name is UTF-8.
const FLAGS: u16 = (1 << 3) | (1 << 11);
const MAX32: u64 = 0xFFFF_FFFF;

struct Entry {
    name: Vec<u8>,
    crc: u32,
    size: u64,
    offset: u64,
    zip64: bool,
    dos_time: u16,
    dos_date: u16,
}

/// Feed it entries one at a time: `begin`, the bytes (through `track`), `end`; then
/// `finish`. Every method returns the bytes to send next.
pub struct ZipStream {
    offset: u64,
    entries: Vec<Entry>,
    current: Option<(Entry, crc32fast::Hasher)>,
}

fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// MS-DOS date and time, the only kind the basic header has.
fn dos_datetime(at: chrono::NaiveDateTime) -> (u16, u16) {
    use chrono::{Datelike, Timelike};
    let year = at.year().clamp(1980, 2107) as u16;
    let time = ((at.hour() as u16) << 11) | ((at.minute() as u16) << 5) | (at.second() as u16 / 2);
    let date = ((year - 1980) << 9) | ((at.month() as u16) << 5) | at.day() as u16;
    (time, date)
}

impl Default for ZipStream {
    fn default() -> Self {
        Self::new()
    }
}

impl ZipStream {
    pub fn new() -> Self {
        ZipStream {
            offset: 0,
            entries: Vec::new(),
            current: None,
        }
    }

    /// The local header of the next entry. `size` is what the entry will hold; past 4 GiB
    /// the entry is written as ZIP64 from the start.
    pub fn begin(&mut self, name: &str, size: u64, modified: chrono::NaiveDateTime) -> Vec<u8> {
        assert!(self.current.is_none(), "end the previous entry first");
        let zip64 = size >= MAX32 || self.offset >= MAX32;
        let (dos_time, dos_date) = dos_datetime(modified);
        let entry = Entry {
            name: name.as_bytes().to_vec(),
            crc: 0,
            size: 0,
            offset: self.offset,
            zip64,
            dos_time,
            dos_date,
        };
        let mut out = Vec::with_capacity(30 + entry.name.len() + 20);
        put32(&mut out, LOCAL_HEADER);
        put16(&mut out, if zip64 { 45 } else { 20 });
        put16(&mut out, FLAGS);
        put16(&mut out, 0); // stored
        put16(&mut out, dos_time);
        put16(&mut out, dos_date);
        put32(&mut out, 0); // crc: in the descriptor
        if zip64 {
            put32(&mut out, MAX32 as u32);
            put32(&mut out, MAX32 as u32);
        } else {
            put32(&mut out, 0);
            put32(&mut out, 0);
        }
        put16(&mut out, entry.name.len() as u16);
        put16(&mut out, if zip64 { 20 } else { 0 });
        out.extend_from_slice(&entry.name);
        if zip64 {
            put16(&mut out, 0x0001);
            put16(&mut out, 16);
            put64(&mut out, 0);
            put64(&mut out, 0);
        }
        self.offset += out.len() as u64;
        self.current = Some((entry, crc32fast::Hasher::new()));
        out
    }

    /// Records bytes of the current entry as they go out unchanged.
    pub fn track(&mut self, chunk: &[u8]) {
        let (entry, hasher) = self.current.as_mut().expect("begin an entry first");
        hasher.update(chunk);
        entry.size += chunk.len() as u64;
        self.offset += chunk.len() as u64;
    }

    /// The data descriptor closing the current entry.
    pub fn end(&mut self) -> Vec<u8> {
        let (mut entry, hasher) = self.current.take().expect("begin an entry first");
        entry.crc = hasher.finalize();
        let mut out = Vec::with_capacity(24);
        put32(&mut out, DATA_DESCRIPTOR);
        put32(&mut out, entry.crc);
        if entry.zip64 {
            put64(&mut out, entry.size);
            put64(&mut out, entry.size);
        } else {
            put32(&mut out, entry.size as u32);
            put32(&mut out, entry.size as u32);
        }
        self.offset += out.len() as u64;
        self.entries.push(entry);
        out
    }

    /// The central directory and the end records.
    pub fn finish(mut self) -> Vec<u8> {
        assert!(self.current.is_none(), "end the last entry first");
        let cd_start = self.offset;
        let mut out = Vec::new();
        for e in &self.entries {
            let big_size = e.size >= MAX32;
            let big_offset = e.offset >= MAX32;
            let mut extra = Vec::new();
            if big_size {
                put64(&mut extra, e.size);
                put64(&mut extra, e.size);
            }
            if big_offset {
                put64(&mut extra, e.offset);
            }
            put32(&mut out, CENTRAL_HEADER);
            put16(&mut out, 45); // made by
            put16(
                &mut out,
                if e.zip64 || big_size || big_offset {
                    45
                } else {
                    20
                },
            );
            put16(&mut out, FLAGS);
            put16(&mut out, 0);
            put16(&mut out, e.dos_time);
            put16(&mut out, e.dos_date);
            put32(&mut out, e.crc);
            let size32 = if big_size {
                MAX32 as u32
            } else {
                e.size as u32
            };
            put32(&mut out, size32);
            put32(&mut out, size32);
            put16(&mut out, e.name.len() as u16);
            put16(
                &mut out,
                if extra.is_empty() {
                    0
                } else {
                    extra.len() as u16 + 4
                },
            );
            put16(&mut out, 0); // comment
            put16(&mut out, 0); // disk
            put16(&mut out, 0); // internal attributes
            put32(&mut out, 0); // external attributes
            put32(
                &mut out,
                if big_offset {
                    MAX32 as u32
                } else {
                    e.offset as u32
                },
            );
            out.extend_from_slice(&e.name);
            if !extra.is_empty() {
                put16(&mut out, 0x0001);
                put16(&mut out, extra.len() as u16);
                out.extend_from_slice(&extra);
            }
        }
        let cd_size = out.len() as u64;
        let count = self.entries.len() as u64;
        let needs64 = count >= 0xFFFF || cd_start >= MAX32 || cd_size >= MAX32;
        if needs64 {
            let zip64_end_at = cd_start + cd_size;
            put32(&mut out, ZIP64_END);
            put64(&mut out, 44);
            put16(&mut out, 45);
            put16(&mut out, 45);
            put32(&mut out, 0);
            put32(&mut out, 0);
            put64(&mut out, count);
            put64(&mut out, count);
            put64(&mut out, cd_size);
            put64(&mut out, cd_start);
            put32(&mut out, ZIP64_LOCATOR);
            put32(&mut out, 0);
            put64(&mut out, zip64_end_at);
            put32(&mut out, 1);
        }
        put32(&mut out, END_OF_CENTRAL);
        put16(&mut out, 0);
        put16(&mut out, 0);
        let count16 = if needs64 { 0xFFFF } else { count as u16 };
        put16(&mut out, count16);
        put16(&mut out, count16);
        put32(
            &mut out,
            if needs64 {
                MAX32 as u32
            } else {
                cd_size as u32
            },
        );
        put32(
            &mut out,
            if needs64 {
                MAX32 as u32
            } else {
                cd_start as u32
            },
        );
        put16(&mut out, 0);
        self.offset += out.len() as u64;
        out
    }
}

/// A file name safe inside an archive: no directories smuggled in, no control characters.
pub fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "file".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn when() -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap()
            .and_hms_opt(14, 30, 10)
            .unwrap()
    }

    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = ZipStream::new();
        let mut out = Vec::new();
        for (name, bytes) in files {
            out.extend(zip.begin(name, bytes.len() as u64, when()));
            zip.track(bytes);
            out.extend_from_slice(bytes);
            out.extend(zip.end());
        }
        out.extend(zip.finish());
        out
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn entries_are_stored_with_a_descriptor_and_indexed_at_the_end() {
        let zip = archive(&[("a.jpg", b"hello"), ("dir/b.jpg", b"world!")]);
        assert_eq!(u32_at(&zip, 0), LOCAL_HEADER);
        // The end record names two entries and points at the central directory.
        let eocd = zip.len() - 22;
        assert_eq!(u32_at(&zip, eocd), END_OF_CENTRAL);
        assert_eq!(u16::from_le_bytes([zip[eocd + 10], zip[eocd + 11]]), 2);
        let cd_start = u32_at(&zip, eocd + 16) as usize;
        assert_eq!(u32_at(&zip, cd_start), CENTRAL_HEADER);
        // The first entry's CRC in the central directory is the CRC of its bytes.
        assert_eq!(u32_at(&zip, cd_start + 16), crc32fast::hash(b"hello"));
        assert_eq!(u32_at(&zip, cd_start + 20), 5);
        // Local header (30) + name (5) + data (5), then the descriptor.
        assert_eq!(u32_at(&zip, 40), DATA_DESCRIPTOR);
        assert_eq!(u32_at(&zip, 44), crc32fast::hash(b"hello"));
    }

    #[test]
    fn an_empty_archive_is_just_the_end_record() {
        let zip = ZipStream::new().finish();
        assert_eq!(zip.len(), 22);
        assert_eq!(u32_at(&zip, 0), END_OF_CENTRAL);
    }

    #[test]
    fn dos_time_counts_two_second_steps() {
        let (time, date) = dos_datetime(when());
        assert_eq!(time, (14 << 11) | (30 << 5) | 5);
        assert_eq!(date, ((2026 - 1980) << 9) | (10 << 5) | 6);
    }

    #[test]
    fn names_cannot_climb_out_of_the_archive() {
        assert_eq!(safe_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_name("  .. "), "file");
        assert_eq!(safe_name("Bal oldal.jpg"), "Bal oldal.jpg");
    }
}
