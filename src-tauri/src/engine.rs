//! The compression engine: MkPFS plus the deep verifier, as one `prospack-engine` program.
//!
//! Release builds ship it frozen as a Tauri sidecar next to the app. Developers can point
//! `PROSPACK_ENGINE` at a frozen build, or let the app run `engine/prospack_engine.py` in a
//! private venv ("Install MkPFS" in the UI).

use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Pinned so a new MkPFS release can't silently change the image format.
pub const MKPFS_VERSION: &str = "1.1.0";
const EXE: &str = if cfg!(windows) { ".exe" } else { "" };
/// The repo's engine sources, for the developer fallback.
const DEV_ENGINE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../engine");

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub python: Option<String>,
    pub mkpfs: Option<String>,
    pub ready: bool,
    /// True when the frozen engine shipped with the app is in use.
    pub bundled: bool,
    /// Whether "Install MkPFS" (developer venv) is possible on this machine.
    pub can_install: bool,
    pub dir: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Runner {
    Frozen(PathBuf),
    Script { python: PathBuf, script: PathBuf },
}

pub struct Engine {
    root: PathBuf,
}

impl Engine {
    pub fn new(data_dir: &Path) -> Self {
        Self { root: data_dir.join("engine") }
    }

    fn venv(&self) -> PathBuf {
        self.root.join("venv")
    }
    fn python_bin(&self) -> PathBuf {
        if cfg!(windows) {
            self.venv().join("Scripts/python.exe")
        } else {
            self.venv().join("bin/python")
        }
    }
    fn dev_script() -> Option<PathBuf> {
        std::env::var_os("PROSPACK_ENGINE_SCRIPT")
            .map(PathBuf::from)
            .or_else(|| Some(PathBuf::from(DEV_ENGINE_DIR).join("prospack_engine.py")))
            .filter(|p| p.is_file())
    }

    /// Sidecar next to the app, then $PROSPACK_ENGINE, then the developer venv.
    fn runner(&self) -> Option<Runner> {
        let sidecar = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|d| d.join(format!("prospack-engine{EXE}"))))
            .filter(|p| p.is_file());
        if let Some(p) = sidecar {
            return Some(Runner::Frozen(p));
        }
        if let Some(p) = std::env::var_os("PROSPACK_ENGINE").map(PathBuf::from).filter(|p| p.is_file()) {
            return Some(Runner::Frozen(p));
        }
        let python = self.python_bin();
        match (python.is_file(), Self::dev_script()) {
            (true, Some(script)) => Some(Runner::Script { python, script }),
            _ => None,
        }
    }

    fn command(&self) -> Result<Command, String> {
        match self.runner() {
            Some(Runner::Frozen(p)) => Ok(quiet(p)),
            Some(Runner::Script { python, script }) => {
                let mut c = quiet(python);
                c.arg(script);
                Ok(c)
            }
            None => Err("The compression engine is missing. Reinstall Prospack.".into()),
        }
    }

    pub fn status(&self) -> EngineStatus {
        let runner = self.runner();
        let mkpfs = self
            .command()
            .ok()
            .and_then(|mut c| c.arg("--version").output().ok())
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().trim_start_matches("mkpfs ").to_string());
        let can_install = Self::dev_script().is_some();
        let python = if matches!(runner, Some(Runner::Frozen(_))) || !can_install {
            None
        } else {
            quiet(if cfg!(windows) { "python" } else { "python3" })
                .arg("--version")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().replace("Python ", ""))
        };
        EngineStatus {
            python,
            ready: mkpfs.is_some(),
            mkpfs,
            bundled: matches!(runner, Some(Runner::Frozen(_))),
            can_install,
            dir: self.root.to_string_lossy().into_owned(),
        }
    }

    /// Developer setup: a venv with the engine's requirements. Release builds never need this.
    pub fn install(&self, log: &mut dyn FnMut(String)) -> Result<(), String> {
        let script = Self::dev_script()
            .ok_or("The compression engine ships with Prospack. Reinstall Prospack to restore it.")?;
        let reqs = script.with_file_name("requirements.txt");
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        if !self.python_bin().is_file() {
            log("Creating Python environment".into());
            run_logged(quiet(if cfg!(windows) { "python" } else { "python3" }).arg("-m").arg("venv").arg(self.venv()), log)?;
        }
        log(format!("Installing MkPFS {MKPFS_VERSION}"));
        run_logged(
            quiet(self.python_bin())
                .args(["-m", "pip", "install", "--disable-pip-version-check", "-r"])
                .arg(reqs),
            log,
        )?;
        Ok(())
    }

    /// `mkpfs pack folder` into `out`. The inner exFAT is named after `out`'s stem,
    /// so `out` must already be called `<TITLE>.ffpfsc`.
    pub fn pack(
        &self,
        src: &Path,
        out: &Path,
        level: u8,
        cpus: u32,
        cancel: &Arc<AtomicBool>,
        on: &mut dyn FnMut(PackEvent),
    ) -> Result<(), String> {
        let tmp_dir = out.parent().unwrap().join("tmp");
        std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;
        let mut cmd = self.command()?;
        cmd.args(["pack", "folder", "--no-adjust-output-file-extension", "--ampr-skip-regen-if-exists"])
            .arg("--compression-level")
            .arg(level.min(9).to_string())
            .arg("--cpu-count")
            .arg(cpus.to_string())
            .arg("--temp-folder")
            .arg(&tmp_dir)
            .arg(src)
            .arg(out);
        let mut parser = PackParser::default();
        let status = run_streaming(cmd, cancel, &mut |chunk| {
            for ev in parser.feed(chunk) {
                on(ev);
            }
        })?;
        let _ = std::fs::remove_dir_all(&tmp_dir);
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        if !status {
            return Err(parser.last_error().unwrap_or_else(|| "MkPFS failed; see the activity log".into()));
        }
        Ok(())
    }

    pub fn verify(
        &self,
        image: &Path,
        src: &Path,
        workers: u32,
        cancel: &Arc<AtomicBool>,
        on: &mut dyn FnMut(VerifyEvent),
    ) -> Result<bool, String> {
        let workers = if workers == 0 {
            (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2) / 2).clamp(1, 8) as u32
        } else {
            workers
        };
        let mut cmd = self.command()?;
        cmd.arg("verify").arg(image).arg(src).arg(workers.to_string());
        let mut buf = String::new();
        let mut verdict: Option<bool> = None;
        run_streaming(cmd, cancel, &mut |chunk| {
            buf.push_str(chunk);
            while let Some(nl) = buf.find('\n') {
                let line: String = buf.drain(..=nl).collect();
                if let Some(ev) = VerifyEvent::parse(line.trim()) {
                    if let VerifyEvent::Done { ok, .. } = ev {
                        verdict = Some(ok);
                    }
                    on(ev);
                }
            }
        })?;
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        verdict.ok_or_else(|| "The verifier stopped without a result".into())
    }
}

/// Puts the child in its own process group (Unix) / new process group without a console
/// window (Windows), so Cancel can stop it and everything it spawned.
fn detach_group(cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
}

#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Stops a process and its children. `force` escalates on Unix (SIGKILL after SIGTERM).
fn kill_tree(pid: u32, force: bool) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), if force { libc::SIGKILL } else { libc::SIGTERM });
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = force;
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
}

/// A command that won't flash a console window on Windows.
fn quiet(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

fn run_logged(cmd: &mut Command, log: &mut dyn FnMut(String)) -> Result<(), String> {
    let out = cmd.output().map_err(|e| e.to_string())?;
    for line in String::from_utf8_lossy(&out.stdout).lines().chain(String::from_utf8_lossy(&out.stderr).lines()) {
        if !line.trim().is_empty() {
            log(line.to_string());
        }
    }
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("{:?} exited with {}", cmd.get_program(), out.status))
    }
}

/// Runs `cmd` in its own process group, feeding merged stdout+stderr to `on`.
/// Kills the whole group (MkPFS and verifier workers) when `cancel` flips.
fn run_streaming(mut cmd: Command, cancel: &Arc<AtomicBool>, on: &mut dyn FnMut(&str)) -> Result<bool, String> {
    cmd.env("PYTHONUNBUFFERED", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("COLUMNS", "120")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    detach_group(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("Couldn't start {:?}: {e}", cmd.get_program()))?;
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    let mut readers = Vec::new();
    for mut pipe in [
        Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
        Box::new(child.stderr.take().unwrap()) as Box<dyn Read + Send>,
    ] {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            let mut buf = [0u8; 16384];
            let mut pending: Vec<u8> = Vec::new();
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        pending.extend_from_slice(&buf[..n]);
                        // Only hand over complete UTF-8.
                        let valid = match std::str::from_utf8(&pending) {
                            Ok(_) => pending.len(),
                            Err(e) => e.valid_up_to(),
                        };
                        let s = String::from_utf8_lossy(&pending[..valid]).into_owned();
                        pending.drain(..valid);
                        let _ = tx.send(Some(s));
                    }
                }
            }
            let _ = tx.send(None);
        }));
    }
    drop(tx);
    let mut open = 2;
    let mut killed = false;
    while open > 0 {
        match rx.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(Some(s)) => on(&s),
            Ok(None) => open -= 1,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
        if !killed && cancel.load(Ordering::Relaxed) {
            kill_tree(pid, false);
            killed = true;
        }
    }
    for r in readers {
        let _ = r.join();
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if killed {
        kill_tree(pid, true);
    }
    Ok(status.success())
}

// ---------------------------------------------------------------------------
// MkPFS output parsing

#[derive(Debug, Clone, PartialEq)]
pub enum PackEvent {
    /// Percentage of the current MkPFS phase, its name, and MB/s when shown.
    Progress { pct: f64, phase: String, mbps: Option<f64> },
    /// Uncompressed size of the inner exFAT image, in bytes.
    InnerSize(u64),
    Line(String),
}

#[derive(Default)]
struct PackParser {
    partial: String,
    errors: Vec<String>,
}

impl PackParser {
    fn feed(&mut self, chunk: &str) -> Vec<PackEvent> {
        self.partial.push_str(chunk);
        let mut out = Vec::new();
        // MkPFS redraws its bar with '\r'; ordinary messages end in '\n'.
        while let Some(i) = self.partial.find(['\r', '\n']) {
            let piece: String = self.partial.drain(..=i).collect();
            let piece = piece.trim_end_matches(['\r', '\n']).trim();
            if piece.is_empty() {
                continue;
            }
            if let Some(ev) = parse_bar(piece) {
                out.push(ev);
                continue;
            }
            if let Some(sz) = parse_inner_size(piece) {
                out.push(PackEvent::InnerSize(sz));
            }
            if piece.contains("❌") || piece.to_ascii_lowercase().starts_with("error") {
                self.errors.push(piece.to_string());
            }
            if !piece.chars().all(|c| c == '=' || c == '-') {
                out.push(PackEvent::Line(piece.to_string()));
            }
        }
        out
    }

    fn last_error(&self) -> Option<String> {
        self.errors.last().cloned()
    }
}

/// `[######-----]  21% compress @ 750.66 MB/s ETA 0s`
fn parse_bar(s: &str) -> Option<PackEvent> {
    let rest = s.strip_prefix('[')?;
    let close = rest.find(']')?;
    let rest = rest[close + 1..].trim_start();
    let pct_end = rest.find('%')?;
    let pct: f64 = rest[..pct_end].trim().parse().ok()?;
    let rest = rest[pct_end + 1..].trim();
    let (phase, tail) = match rest.find(" @ ") {
        Some(i) => (&rest[..i], Some(&rest[i + 3..])),
        None => (rest, None),
    };
    let mbps = tail.and_then(|t| {
        let num: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        let v: f64 = num.parse().ok()?;
        let unit = t[num.len()..].trim_start();
        Some(if unit.starts_with("GB/s") { v * 1000.0 } else if unit.starts_with("KB/s") { v / 1000.0 } else { v })
    });
    Some(PackEvent::Progress { pct, phase: phase.trim().to_string(), mbps })
}

/// `Inner image: PPSA00001.exfat (356.69 MB)` → bytes (decimal MB as MkPFS prints them).
fn parse_inner_size(s: &str) -> Option<u64> {
    let rest = s.strip_prefix("Inner image:")?;
    let open = rest.rfind('(')?;
    let inner = rest[open + 1..].trim_end_matches(')');
    let mut it = inner.split_whitespace();
    let v: f64 = it.next()?.parse().ok()?;
    let mult = match it.next()? {
        "B" => 1.0,
        "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        _ => return None,
    };
    Some((v * mult) as u64)
}

// ---------------------------------------------------------------------------
// Verifier output

#[derive(Debug, Clone, PartialEq)]
pub enum VerifyEvent {
    Plan { files: u64, bytes: u64 },
    Progress { files: u64, bytes: u64 },
    Problem { kind: String, path: String, detail: String },
    Done { ok: bool, files: u64, problems: u64 },
}

impl VerifyEvent {
    fn parse(line: &str) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(line).ok()?;
        let n = |k: &str| v[k].as_u64().unwrap_or(0);
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        Some(match v["type"].as_str()? {
            "plan" => Self::Plan { files: n("files"), bytes: n("bytes") },
            "progress" => Self::Progress { files: n("files"), bytes: n("bytes") },
            "problem" => Self::Problem { kind: s("kind"), path: s("path"), detail: s("detail") },
            "done" => Self::Done { ok: v["ok"].as_bool().unwrap_or(false), files: n("files"), problems: n("problems") },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_bars() {
        assert_eq!(
            parse_bar("[######--------------------------]  21% compress @ 750.66 MB/s ETA 0s"),
            Some(PackEvent::Progress { pct: 21.0, phase: "compress".into(), mbps: Some(750.66) })
        );
        assert_eq!(
            parse_bar("[--------------------------------]   0% compress"),
            Some(PackEvent::Progress { pct: 0.0, phase: "compress".into(), mbps: None })
        );
        assert_eq!(parse_bar("Compressing inner exFAT (356.69 MB)..."), None);
    }

    #[test]
    fn splits_carriage_return_redraws() {
        let mut p = PackParser::default();
        let evs = p.feed("Inner image: PPSA00001.exfat (356.69 MB)\n\r[#---]   5% compress @ 1.20 GB/s ETA 3s\r[##--]  10% comp");
        assert_eq!(evs[0], PackEvent::InnerSize(356_690_000));
        assert!(matches!(evs[1], PackEvent::Line(_)));
        assert_eq!(evs[2], PackEvent::Progress { pct: 5.0, phase: "compress".into(), mbps: Some(1200.0) });
        assert_eq!(evs.len(), 3, "partial bar must wait for the rest");
        let evs = p.feed("ress @ 800.00 MB/s ETA 1s\r");
        assert_eq!(evs, vec![PackEvent::Progress { pct: 10.0, phase: "compress".into(), mbps: Some(800.0) }]);
    }

    #[test]
    fn parses_verifier_lines() {
        assert_eq!(
            VerifyEvent::parse(r#"{"type": "problem", "kind": "content", "path": "eboot.bin", "detail": "first difference at byte 12"}"#),
            Some(VerifyEvent::Problem { kind: "content".into(), path: "eboot.bin".into(), detail: "first difference at byte 12".into() })
        );
        assert_eq!(
            VerifyEvent::parse(r#"{"type": "done", "ok": true, "files": 409, "bytes": 1, "problems": 0}"#),
            Some(VerifyEvent::Done { ok: true, files: 409, problems: 0 })
        );
    }
}

#[cfg(test)]
mod cancel_test {
    use super::*;

    /// PACKER_E2E_SCRATCH=<dir with mkvenv> cargo test cancel -- --ignored
    #[test]
    #[ignore]
    #[cfg(unix)]
    fn cancel_kills_mkpfs_promptly() {
        let scratch = PathBuf::from(std::env::var("PACKER_E2E_SCRATCH").unwrap());
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("engine");
        std::fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(scratch.join("mkvenv"), root.join("venv")).unwrap();
        let src = tmp.path().join("PPSA00002-app");
        std::fs::create_dir_all(src.join("sce_sys")).unwrap();
        std::fs::write(src.join("sce_sys/param.json"), r#"{"titleId":"PPSA00002"}"#).unwrap();
        // 3 GB of compressible data: long enough to cancel mid-way.
        let chunk = vec![b'x'; 64 << 20];
        let mut f = std::fs::File::create(src.join("big.bin")).unwrap();
        for _ in 0..48 {
            std::io::Write::write_all(&mut f, &chunk).unwrap();
        }
        drop(f);
        let out_dir = tmp.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let engine = Engine { root };
        assert!(engine.runner().is_some(), "no engine: set PROSPACK_ENGINE or provide the venv");
        let cancel = Arc::new(AtomicBool::new(false));
        let c2 = cancel.clone();
        let t = std::time::Instant::now();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            c2.store(true, Ordering::Relaxed);
        });
        let mut saw_progress = false;
        let res = engine.pack(&src, &out_dir.join("PPSA00002.ffpfsc"), 7, 0, &cancel, &mut |ev| {
            if matches!(ev, PackEvent::Progress { .. }) {
                saw_progress = true;
            }
        });
        assert_eq!(res, Err("Cancelled".into()));
        assert!(t.elapsed().as_secs() < 8, "cancel took {:?}", t.elapsed());
        std::thread::sleep(std::time::Duration::from_millis(300));
        let left = Command::new("pgrep").args(["-f", src.to_str().unwrap()]).output().unwrap();
        assert!(left.stdout.is_empty(), "mkpfs still running: {}", String::from_utf8_lossy(&left.stdout));
        eprintln!("progress seen: {saw_progress}, cancelled after {:?}", t.elapsed());
    }
}
