//! Everything that talks to the console: connection checks, ShadowMountPlus hints, mount checks.

use crate::ftp::Ftp;
use crate::settings::Settings;
use serde::Serialize;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::{Duration, Instant};

pub const SMP_DIR: &str = "/data/shadowmount";
/// Game Compressor writes the same value for PFSC containers.
pub const PFSC_SECTOR: u32 = 65536;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Ps5Status {
    pub online: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub free_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub shadowmount: bool,
}

pub fn connect(s: &Settings) -> Result<Ftp, String> {
    Ftp::connect(&s.ps5_host, s.ftp_port, Duration::from_secs(4))
}

/// Read-only check: FTP login + NOOP, ShadowMountPlus config present, and free space from
/// the web file manager if it's running.
pub fn status(s: &Settings) -> Ps5Status {
    let t0 = Instant::now();
    let mut ftp = match connect(s) {
        Ok(f) => f,
        Err(e) => return Ps5Status { error: Some(e), ..Default::default() },
    };
    if let Err(e) = ftp.noop() {
        return Ps5Status { error: Some(e), ..Default::default() };
    }
    let latency_ms = Some(t0.elapsed().as_millis() as u64);
    let shadowmount = ftp.size(&format!("{SMP_DIR}/config.ini")).ok().flatten().is_some();
    ftp.quit();
    let (free_bytes, total_bytes) = space(s).unwrap_or((None, None));
    Ps5Status { online: true, latency_ms, error: None, free_bytes, total_bytes, shadowmount }
}

/// GET /api/space?path=/data on the PS5 web file manager. Optional; a missing server is fine.
fn space(s: &Settings) -> Option<(Option<u64>, Option<u64>)> {
    let body = http_get(&s.ps5_host, s.web_port, "/api/space?path=/data")?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    let sp = v["spaces"].as_array()?.iter().find(|x| x["path"] == "/data")?;
    Some((sp["free"].as_u64(), sp["total"].as_u64()))
}

fn http_get(host: &str, port: u16, path: &str) -> Option<String> {
    let addr = (host, port).to_socket_addrs().ok()?.next()?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(4))).ok();
    write!(s, "GET {path} HTTP/1.0\r\nHost: {host}\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n").ok()?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).ok()?;
    let raw = String::from_utf8_lossy(&raw);
    let (head, body) = raw.split_once("\r\n\r\n")?;
    head.starts_with("HTTP/1.").then(|| ())?;
    head.split_whitespace().nth(1).filter(|c| *c == "200")?;
    Some(body.to_string())
}

/// Other sources for the same title already in the upload folder (e.g. a loose `-app` folder),
/// which ShadowMountPlus could mount instead of the new image.
pub fn competitors(ftp: &mut Ftp, remote_dir: &str, title_id: &str) -> Result<Vec<String>, String> {
    Ok(ftp
        .list_names(remote_dir)?
        .into_iter()
        .map(|(n, _)| n)
        .filter(|n| {
            n.len() >= 9
                && n[..9].eq_ignore_ascii_case(title_id)
                && !n.starts_with('.')
                && n != &format!("{title_id}.ffpfsc")
                && !n.ends_with(".vhash")
        })
        .collect())
}

// ---------------------------------------------------------------------------
// ShadowMountPlus hints, matching what Game Compressor writes for a compressed image:
//   autotune.ini: image_sector=<TITLE>.ffpfsc:65536
//   config.ini:   image_ro=<TITLE>.exfat
// Both files are flat key=value; names are basenames compared case-insensitively.

/// Removes `keys` lines whose image name is in `names`, then appends `add` (if any).
/// Returns `None` when nothing would change.
pub fn upsert_hint(content: &str, keys: &[&str], names: &[String], add: Option<&str>) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    let mut have_add = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if Some(trimmed) == add && !have_add {
            have_add = true;
            out.push(line.to_string());
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let k = k.trim();
            if keys.contains(&k) {
                let v = v.trim();
                // image_sector values are NAME:SECTOR (split on the last colon).
                let name = if k == "image_sector" { v.rsplit_once(':').map(|x| x.0).unwrap_or(v) } else { v };
                let name = name.rsplit('/').next().unwrap_or(name).trim();
                if names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
                    continue;
                }
            }
        }
        out.push(line.to_string());
    }
    if let (Some(a), false) = (add, have_add) {
        out.push(a.to_string());
    }
    let mut next = out.join("\n");
    if !next.is_empty() {
        next.push('\n');
    }
    let same = next.trim_end() == content.trim_end();
    (!same).then_some(next)
}

pub struct HintResult {
    pub changed: Vec<String>,
    pub backups: Vec<String>,
}

pub fn write_hints(ftp: &mut Ftp, title_id: &str, backup_dir: &Path) -> Result<HintResult, String> {
    let outer = format!("{title_id}.ffpfsc");
    let nested = format!("{title_id}.exfat");
    let names = vec![outer.clone(), nested.clone(), format!("{title_id}.pfs")];
    let plan: [(&str, Vec<&str>, String); 2] = [
        ("autotune.ini", vec!["image_sector"], format!("image_sector={outer}:{PFSC_SECTOR}")),
        ("config.ini", vec!["image_ro", "image_rw"], format!("image_ro={nested}")),
    ];
    let mut res = HintResult { changed: vec![], backups: vec![] };
    std::fs::create_dir_all(backup_dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    for (file, keys, line) in plan {
        let path = format!("{SMP_DIR}/{file}");
        let current = ftp.retr(&path)?;
        if current.is_none() && file == "config.ini" {
            return Err("ShadowMountPlus isn't set up on this PS5 (no /data/shadowmount/config.ini)".into());
        }
        let text = String::from_utf8_lossy(current.as_deref().unwrap_or_default()).into_owned();
        let Some(next) = upsert_hint(&text, &keys, &names, Some(&line)) else { continue };
        if let Some(bytes) = &current {
            let b = backup_dir.join(format!("{stamp}-{file}"));
            std::fs::write(&b, bytes).map_err(|e| e.to_string())?;
            res.backups.push(b.to_string_lossy().into_owned());
        }
        if ftp.put_atomic(&path, next.as_bytes()).is_err() {
            // Some servers refuse to rename over an existing file.
            ftp.delete(&path).ok();
            let (dir, name) = path.rsplit_once('/').unwrap();
            ftp.rename(&format!("{dir}/.{name}.packer-tmp"), &path)?;
        }
        res.changed.push(format!("{file}: {line}"));
    }
    Ok(res)
}

/// Polls `/user/app/<TITLE>/mount_img.lnk` until ShadowMountPlus records the image.
pub fn wait_for_mount(
    s: &Settings,
    title_id: &str,
    image_remote: &str,
    timeout: Duration,
    cancel: &std::sync::atomic::AtomicBool,
    tick: &mut dyn FnMut(u64),
) -> Result<bool, String> {
    let start = Instant::now();
    let link = format!("/user/app/{title_id}/mount_img.lnk");
    while start.elapsed() < timeout {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        tick(start.elapsed().as_secs());
        if let Ok(mut ftp) = connect(s) {
            if let Ok(Some(bytes)) = ftp.retr(&link) {
                let text = String::from_utf8_lossy(&bytes);
                if text.lines().any(|l| l.trim() == image_remote) {
                    ftp.quit();
                    return Ok(true);
                }
            }
            ftp.quit();
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        vec!["PPSA01234.ffpfsc".into(), "PPSA01234.exfat".into(), "PPSA01234.pfs".into()]
    }

    #[test]
    fn adds_hint_and_keeps_other_lines() {
        let before = "# comment\nscan_interval=15\nimage_ro=OTHER.exfat\n";
        let after = upsert_hint(before, &["image_ro", "image_rw"], &names(), Some("image_ro=PPSA01234.exfat")).unwrap();
        assert_eq!(after, "# comment\nscan_interval=15\nimage_ro=OTHER.exfat\nimage_ro=PPSA01234.exfat\n");
    }

    #[test]
    fn replaces_stale_hints_for_the_same_title() {
        let before = "image_sector=ppsa01234.FFPFSC:4096\nimage_sector=PPSA01234.exfat:8192\nimage_sector=X.ffpfsc:65536\n";
        let after =
            upsert_hint(before, &["image_sector"], &names(), Some("image_sector=PPSA01234.ffpfsc:65536")).unwrap();
        assert_eq!(after, "image_sector=X.ffpfsc:65536\nimage_sector=PPSA01234.ffpfsc:65536\n");
    }

    #[test]
    fn unchanged_when_already_correct() {
        let before = "a=1\nimage_ro=PPSA01234.exfat\n";
        assert!(upsert_hint(before, &["image_ro", "image_rw"], &names(), Some("image_ro=PPSA01234.exfat")).is_none());
    }

    #[test]
    fn drops_rw_rule_that_would_conflict() {
        let before = "image_rw=PPSA01234.exfat\n";
        let after = upsert_hint(before, &["image_ro", "image_rw"], &names(), Some("image_ro=PPSA01234.exfat")).unwrap();
        assert_eq!(after, "image_ro=PPSA01234.exfat\n");
    }
}
