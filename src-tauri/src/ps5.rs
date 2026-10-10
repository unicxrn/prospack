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
    /// /data/GameCompressor exists (optional; used for the validated badge).
    pub game_compressor: bool,
}

pub fn connect(s: &Settings) -> Result<Ftp, String> {
    if s.ps5_host.trim().is_empty() {
        return Err("No PS5 address set. Open Settings → Run setup again, or enter your PS5's IP in Settings.".into());
    }
    Ftp::connect(s.ps5_host.trim(), s.ftp_port, Duration::from_secs(4))
}

/// Read-only check: FTP login + NOOP, ShadowMountPlus config present, and free space from
/// the web file manager if it's running.
pub fn status(s: &Settings) -> Ps5Status {
    let t0 = Instant::now();
    let mut ftp = match connect(s) {
        Ok(f) => f,
        Err(e) => return Ps5Status { error: Some(e), ..Default::default() },
    };
    // (connect() already rejects an empty address with a helpful message.)
    if let Err(e) = ftp.noop() {
        return Ps5Status { error: Some(e), ..Default::default() };
    }
    let latency_ms = Some(t0.elapsed().as_millis() as u64);
    let shadowmount = ftp.size(&format!("{SMP_DIR}/config.ini")).ok().flatten().is_some();
    let game_compressor = ftp
        .list("/data")
        .map(|entries| entries.iter().any(|e| e.is_dir && e.name == "GameCompressor"))
        .unwrap_or(false);
    ftp.quit();
    let (free_bytes, total_bytes) = space(s).unwrap_or((None, None));
    Ps5Status { online: true, latency_ms, error: None, free_bytes, total_bytes, shadowmount, game_compressor }
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
            return Err("ShadowMountPlus isn't set up on this PS5 (no /data/shadowmount/config.ini). Load ShadowMountPlus on the PS5, then use Check again.".into());
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
            ftp.rename(&format!("{dir}/.{name}.prospack-tmp"), &path)?;
        }
        res.changed.push(format!("{file}: {line}"));
    }
    Ok(res)
}

// ---------------------------------------------------------------------------
// Game Compressor's validation record. GC shows a compressed game as "validated", with its
// original size and savings, when /data/GameCompressor/validations/<TITLE>-<fnv1a32(path)>.json
// (or the legacy <TITLE>.json) names this path, its sourceSize equals the image's size, and
// the status isn't a bad-blocks or stats-only one.

pub const GC_VALIDATIONS: &str = "/data/GameCompressor/validations";

pub fn fnv1a32(s: &str) -> u32 {
    s.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

pub struct GcRecord<'a> {
    pub title_id: &'a str,
    pub image_remote: &'a str,
    pub image_bytes: u64,
    pub source_bytes: u64,
    pub apr_indexed: bool,
    pub files_verified: u64,
}

pub fn gc_record_json(r: &GcRecord, now: u64) -> String {
    let v = serde_json::json!({
        "titleId": r.title_id,
        "path": r.image_remote,
        "sourceSize": r.image_bytes,
        "sourceMtime": now,
        "compressionSourceSize": r.source_bytes,
        "compressionCompressedSize": r.image_bytes,
        "compressionSavedBytes": r.source_bytes.saturating_sub(r.image_bytes),
        "aprIndexed": r.apr_indexed,
        "amprHotSwapOptimized": false,
        // Any status other than bad-blocks-found*, compression-stats or not-mounted reads as validated.
        "status": "verified-by-prospack",
        "nestedName": format!("{}.exfat", r.title_id),
        "repairedBlocks": 0,
        "hashMode": "prospack-file-compare",
        "hashCheckedBlocks": 0,
        "hashMatchedBlocks": 0,
        "hashMismatchedBlocks": 0,
        "verifiedBy": "Prospack",
        "verifiedFiles": r.files_verified,
        "updatedAt": now,
    });
    serde_json::to_string_pretty(&v).unwrap() + "\n"
}

pub fn write_gc_record(ftp: &mut Ftp, r: &GcRecord) -> Result<(), String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let body = gc_record_json(r, now);
    ftp.mkdir_p("/data/GameCompressor")?;
    ftp.mkdir_p(GC_VALIDATIONS)?;
    let keyed = format!("{GC_VALIDATIONS}/{}-{:08x}.json", r.title_id, fnv1a32(r.image_remote));
    ftp.put_atomic(&keyed, body.as_bytes())?;
    ftp.put_atomic(&format!("{GC_VALIDATIONS}/{}.json", r.title_id), body.as_bytes())
}

/// manual.lst with `image` added, dropping other entries for the same title (same rules as
/// Game Compressor: exact path, or a basename of TITLE followed by end, '-' or '.').
pub fn manual_list_with(content: &str, title_id: &str, image: &str) -> String {
    let mut out: Vec<&str> = content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| {
            if *l == image {
                return false;
            }
            let base = l.rsplit('/').next().unwrap_or(l);
            let same_title = base.len() >= 9
                && base[..9].eq_ignore_ascii_case(title_id)
                && matches!(base.as_bytes().get(9), None | Some(b'-') | Some(b'.'));
            l.starts_with('#') || !same_title
        })
        .collect();
    out.push(image);
    out.join("\n") + "\n"
}

/// Adds the image to /data/shadowmount/manual.lst. Rewriting that file is what makes
/// ShadowMountPlus scan; a new file in the folder alone may not be picked up for minutes.
pub fn request_scan(ftp: &mut Ftp, title_id: &str, image: &str, backup_dir: Option<&Path>) -> Result<bool, String> {
    let path = format!("{SMP_DIR}/manual.lst");
    let current = ftp.retr(&path)?;
    let text = String::from_utf8_lossy(current.as_deref().unwrap_or_default()).into_owned();
    let next = manual_list_with(&text, title_id, image);
    let changed = next.trim_end() != text.trim_end();
    if changed {
        if let (Some(dir), Some(bytes)) = (backup_dir, &current) {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            std::fs::write(dir.join(format!("{stamp}-manual.lst")), bytes).map_err(|e| e.to_string())?;
        }
    }
    // Rewrite even when unchanged: the replace itself is the scan trigger.
    ftp.put_atomic(&path, next.as_bytes())?;
    Ok(changed)
}

/// How a mount attempt ended.
#[derive(Debug, Clone, PartialEq)]
pub enum MountOutcome {
    /// ShadowMountPlus mounted and registered the game.
    Registered,
    /// ShadowMountPlus tried and gave up; the reason is ready to show the user.
    Failed(String),
    /// Nothing conclusive before the timeout.
    Unconfirmed,
}

const SMP_LOG: &str = "/data/shadowmount/debug.log";

/// True when manual.status says ShadowMountPlus installed this image. It writes that line
/// only after the game is mounted *and* registered; the link files in /user/app are written
/// before registration and stay behind when it fails, so they can't be trusted on their own.
pub fn manual_status_installed(text: &str, title_id: &str, image: &str) -> bool {
    text.lines().any(|l| {
        let f: Vec<&str> = l.split('\t').collect();
        f.len() >= 3 && f[0] == "installed" && f[1].eq_ignore_ascii_case(title_id) && f[2] == image
    })
}

/// Looks through new ShadowMountPlus log lines for a registration failure of this title.
pub fn register_failure(log: &str, title_id: &str) -> Option<String> {
    let mut ours = false;
    for line in log.lines() {
        if line.contains("[ACTION]") {
            // "Installing: Name (TITLE)" / "Remounting: Name": track whether the following lines are about us.
            ours = line.contains(&format!("({title_id})"));
            continue;
        }
        if ours && line.contains("AppInstallTitleDir bridge unavailable") {
            return Some(
                "ShadowMountPlus mounted the game but couldn't register it: its system hook was lost, which happens \
                 after rest mode in ShadowMountPlus 1.7beta4 and older. Update ShadowMountPlus (1.7beta5 fixes it) or \
                 restart it, then use Check again."
                    .into(),
            );
        }
        if ours && line.contains("sceAppInstUtilAppInstallTitleDir unavailable") {
            return Some(
                "ShadowMountPlus couldn't register the game (the PS5's install service wasn't available). Restart \
                 ShadowMountPlus, then use Check again."
                    .into(),
            );
        }
        if line.contains("retry limit reached") && line.contains(&format!("({title_id})")) {
            return Some(
                "ShadowMountPlus gave up on the game after two attempts. Restart ShadowMountPlus, then use Check again. \
                 Its log is /data/shadowmount/debug.log."
                    .into(),
            );
        }
        if ours {
            if let Some(code) = line.split("[REG] FAIL: ").nth(1) {
                return Some(format!(
                    "The PS5 refused to register the game (error {}). Restart ShadowMountPlus and use Check again; \
                     if it keeps failing, the dump may need a backport for your firmware.",
                    code.trim()
                ));
            }
        }
    }
    None
}

fn log_size(ftp: &mut Ftp) -> Option<u64> {
    ftp.size(SMP_LOG).ok().flatten()
}

/// Waits until ShadowMountPlus has registered the image, it reports a failure, or `timeout`.
pub fn wait_for_mount(
    s: &Settings,
    title_id: &str,
    image_remote: &str,
    timeout: Duration,
    cancel: &std::sync::atomic::AtomicBool,
    tick: &mut dyn FnMut(u64),
) -> Result<MountOutcome, String> {
    let start = Instant::now();
    let status_path = format!("{SMP_DIR}/manual.status");
    // Only log lines written from now on are about this attempt.
    let mut log_offset = connect(s).ok().and_then(|mut f| {
        let n = log_size(&mut f);
        f.quit();
        n
    });
    let mut last_nudge = Instant::now();
    while start.elapsed() < timeout {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        tick(start.elapsed().as_secs());
        if let Ok(mut ftp) = connect(s) {
            if let Ok(Some(bytes)) = ftp.retr(&status_path) {
                if manual_status_installed(&String::from_utf8_lossy(&bytes), title_id, image_remote) {
                    ftp.quit();
                    return Ok(MountOutcome::Registered);
                }
            }
            if let Some(size) = log_size(&mut ftp) {
                // The log rotates to debug.log.1 when it grows; start over from 0 then.
                let from = match log_offset {
                    Some(o) if o <= size => o,
                    _ => 0,
                };
                if size > from {
                    if let Ok(Some(new)) = ftp.retr_from(SMP_LOG, from) {
                        if let Some(reason) = register_failure(&String::from_utf8_lossy(&new), title_id) {
                            ftp.quit();
                            return Ok(MountOutcome::Failed(reason));
                        }
                    }
                }
                log_offset = Some(log_offset.map_or(size, |o| if o <= size { o } else { 0 }));
            }
            if last_nudge.elapsed() >= Duration::from_secs(30) {
                last_nudge = Instant::now();
                let _ = request_scan(&mut ftp, title_id, image_remote, None);
            }
            ftp.quit();
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    Ok(MountOutcome::Unconfirmed)
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
    fn manual_status_needs_installed_title_and_path() {
        let st = "installed\tPPSA01234\t/data/homebrew/PPSA01234.ffpfsc\tSample Game\ndeleted\tPPSA05678\t/data/homebrew/PPSA05678.ffpfsc\tX\n";
        assert!(manual_status_installed(st, "PPSA01234", "/data/homebrew/PPSA01234.ffpfsc"));
        assert!(!manual_status_installed(st, "PPSA05678", "/data/homebrew/PPSA05678.ffpfsc"));
        assert!(!manual_status_installed(st, "PPSA01234", "/data/homebrew/PPSA01234-app"));
    }

    #[test]
    fn spots_the_lost_hook_failure_from_the_real_log() {
        // Lines as ShadowMountPlus 1.7beta4 logged them when its hook was gone.
        let log = "[01:45:13]   [ACTION] Installing: Super Meat Boy 3D (PPSA31162)\n\
[01:45:17]   [LINK] mount_img.lnk created: /user/app/PPSA31162/mount_img.lnk -> /data/homebrew/PPSA31162.ffpfsc layers=2\n\
[01:45:17]   [REG] internal AppInstallTitleDir bridge unavailable\n\
[01:45:17] NOTIFY: Register failed: Super Meat Boy 3D (PPSA31162)\n";
        assert!(register_failure(log, "PPSA31162").unwrap().contains("system hook was lost"));
        assert_eq!(register_failure(log, "PPSA01234"), None, "another game's failure isn't ours");
    }

    #[test]
    fn other_register_failures() {
        let fail = "[ACTION] Installing: G (PPSA01234)\n[REG] FAIL: 0x80990015\n";
        assert!(register_failure(fail, "PPSA01234").unwrap().contains("0x80990015"));
        let gave_up = "[SKIP] mount/register retry limit reached (2/2): G (PPSA01234)\n";
        assert!(register_failure(gave_up, "PPSA01234").unwrap().contains("gave up"));
        let ok = "[ACTION] Installing: G (PPSA01234)\n[REG] Installed NEW!\n";
        assert_eq!(register_failure(ok, "PPSA01234"), None);
    }

    #[test]
    fn fnv_matches_game_compressor_file_names() {
        // Same FNV-1a 32 Game Compressor uses for its validations/<TITLE>-<hash>.json names.
        assert_eq!(format!("{:08x}", fnv1a32("/data/homebrew/PPSA01234.ffpfsc")), "844067eb");
        assert_eq!(format!("{:08x}", fnv1a32("/data/homebrew/PPSA05678.ffpfsc")), "17b59d6b");
    }

    #[test]
    fn gc_record_has_the_fields_gc_reads() {
        let r = GcRecord {
            title_id: "PPSA01234",
            image_remote: "/data/homebrew/PPSA01234.ffpfsc",
            image_bytes: 53_800_000_000,
            source_bytes: 86_900_000_000,
            apr_indexed: true,
            files_verified: 35,
        };
        let v: serde_json::Value = serde_json::from_str(&gc_record_json(&r, 1)).unwrap();
        assert_eq!(v["path"], "/data/homebrew/PPSA01234.ffpfsc");
        assert_eq!(v["sourceSize"], 53_800_000_000u64);
        assert_eq!(v["compressionSavedBytes"], 33_100_000_000u64);
        assert_eq!(v["aprIndexed"], true);
        let status = v["status"].as_str().unwrap();
        assert!(!status.starts_with("bad-blocks") && status != "compression-stats" && status != "not-mounted");
    }

    #[test]
    fn manual_list_replaces_same_title_entries() {
        let before = "/data/homebrew/PPSA05678.ffpfsc\n/data/homebrew/PPSA01234-app\n\n/data/homebrew/PPSA012340.ffpfsc\n";
        let after = manual_list_with(before, "PPSA01234", "/data/homebrew/PPSA01234.ffpfsc");
        assert_eq!(
            after,
            "/data/homebrew/PPSA05678.ffpfsc\n/data/homebrew/PPSA012340.ffpfsc\n/data/homebrew/PPSA01234.ffpfsc\n"
        );
        assert_eq!(manual_list_with("", "PPSA01234", "/x/PPSA01234.ffpfsc"), "/x/PPSA01234.ffpfsc\n");
    }

    #[test]
    fn drops_rw_rule_that_would_conflict() {
        let before = "image_rw=PPSA01234.exfat\n";
        let after = upsert_hint(before, &["image_ro", "image_rw"], &names(), Some("image_ro=PPSA01234.exfat")).unwrap();
        assert_eq!(after, "image_ro=PPSA01234.exfat\n");
    }
}
