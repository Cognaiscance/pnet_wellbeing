//! Latest report for each pair of devices. The file is rewritten on each update.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub from_id: [u8; 16],
    pub from_alias: String,
    pub to_id: [u8; 16],
    pub to_alias: String,
    pub who: String,
    pub online: bool,
    pub delay_ms: u32,
    pub bytes_per_sec: u32,
    pub at_ms: u64,
    /// `server`, `lan`, `punch`, `session`, or `unknown`.
    pub path: String,
}

#[derive(Clone, Debug, Default)]
pub struct Store {
    path: PathBuf,
    rows: Vec<Report>,
}

impl Store {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let rows = match fs::read_to_string(&path) {
            Ok(text) => text.lines().filter_map(parse_line).collect(),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(err),
        };
        Ok(Self { path, rows })
    }

    pub fn rows(&self) -> &[Report] {
        &self.rows
    }

    pub fn upsert(&mut self, report: Report) -> io::Result<()> {
        if let Some(existing) = self
            .rows
            .iter_mut()
            .find(|row| row.from_id == report.from_id && row.to_id == report.to_id)
        {
            *existing = report;
        } else {
            self.rows.push(report);
        }
        self.rows.sort_by(|a, b| {
            (&a.from_alias, &a.to_alias, &a.from_id, &a.to_id).cmp(&(
                &b.from_alias,
                &b.to_alias,
                &b.from_id,
                &b.to_id,
            ))
        });
        self.write()
    }

    fn write(&self) -> io::Result<()> {
        let mut body = String::new();
        for row in &self.rows {
            body.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                hex(&row.from_id),
                escape(&row.from_alias),
                hex(&row.to_id),
                escape(&row.to_alias),
                escape(&row.who),
                u8::from(row.online),
                row.delay_ms,
                row.bytes_per_sec,
                row.at_ms,
                escape(&row.path),
            ));
        }
        let tmp = self.path.with_extension("tmp");
        {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(body.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace(['\t', '\n'], " ")
}

fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            out.push(chars.next().unwrap_or('\\'));
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn parse_hex(text: &str) -> Option<[u8; 16]> {
    if text.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, chunk) in text.as_bytes().chunks(2).enumerate() {
        let hi = hex_val(chunk[0])?;
        let lo = hex_val(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_line(line: &str) -> Option<Report> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 9 && parts.len() != 10 {
        return None;
    }
    Some(Report {
        from_id: parse_hex(parts[0])?,
        from_alias: unescape(parts[1]),
        to_id: parse_hex(parts[2])?,
        to_alias: unescape(parts[3]),
        who: unescape(parts[4]),
        online: parts[5] != "0",
        delay_ms: parts[6].parse().ok()?,
        bytes_per_sec: parts[7].parse().ok()?,
        at_ms: parts[8].parse().ok()?,
        path: parts.get(9).map(|text| unescape(text)).unwrap_or_else(|| "unknown".into()),
    })
}

pub fn results_path(dir: &Path) -> PathBuf {
    dir.join("results")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_report_replaces_the_same_pair() {
        let path = std::env::temp_dir().join(format!(
            "wellbeing-store-{}-{}.txt",
            std::process::id(),
            hex(&[0x11; 4])
        ));
        let _ = fs::remove_file(&path);
        let mut store = Store::open(path.clone()).unwrap();
        let mut report = Report {
            from_id: [1; 16],
            from_alias: "laptop".into(),
            to_id: [2; 16],
            to_alias: "n64".into(),
            who: "you".into(),
            online: false,
            delay_ms: 0,
            bytes_per_sec: 0,
            at_ms: 10,
            path: "server".into(),
        };
        store.upsert(report.clone()).unwrap();
        report.online = true;
        report.delay_ms = 12;
        report.bytes_per_sec = 4000;
        report.at_ms = 20;
        store.upsert(report).unwrap();
        let again = Store::open(path.clone()).unwrap();
        assert_eq!(again.rows().len(), 1);
        assert!(again.rows()[0].online);
        assert_eq!(again.rows()[0].delay_ms, 12);
        let _ = fs::remove_file(&path);
    }
}
