//! Drives the real Prospack NSIS installer silently and reports progress.
//! Everything Windows-specific is behind cfg(windows); the logic below is tested anywhere.

use serde::Serialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// NSIS silent install into `dir`. `/D=` must be the last argument and must not be quoted.
pub fn nsis_args(dir: &Path) -> Vec<OsString> {
    let mut d = OsString::from("/D=");
    d.push(dir.as_os_str());
    vec!["/S".into(), d]
}

/// Time-based estimate for when bytes can't be measured (an update overwrites files in place).
/// Assumes ~80 MB/s, at least 4 s, and tops out at 95%.
pub fn time_progress(elapsed_secs: f64, expected: u64) -> f64 {
    let est = (expected as f64 / 80e6).max(4.0);
    (elapsed_secs / est * 100.0).min(95.0)
}

/// Percentage from bytes written so far. Never reports 100 before the installer exits.
pub fn progress(written: u64, expected: u64) -> f64 {
    if expected == 0 {
        return 0.0;
    }
    (written as f64 / expected as f64 * 100.0).clamp(0.0, 99.0)
}

pub fn phase_label(pct: f64) -> &'static str {
    if pct < 70.0 {
        "Extracting Prospack"
    } else if pct < 90.0 {
        "Installing the compression engine"
    } else if pct < 97.0 {
        "Creating shortcuts"
    } else {
        "Registering uninstaller"
    }
}

/// Total size of the files under `dir`.
pub fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => total += e.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

pub fn default_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public\AppData\Local"))
        .join("Programs")
        .join("Prospack")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Existing {
    pub dir: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub exe: String,
    pub message: String,
}

pub struct Options {
    pub dir: PathBuf,
    pub start_menu: bool,
    pub desktop: bool,
}

/// Shortcut locations the Tauri NSIS template may use for the Start menu entry.
pub fn start_menu_links(programs: &Path) -> [PathBuf; 2] {
    [programs.join("Prospack.lnk"), programs.join("Prospack").join("Prospack.lnk")]
}

#[cfg(windows)]
pub fn existing_install() -> Option<Existing> {
    use winreg::enums::HKEY_CURRENT_USER;
    let key = winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Prospack")
        .ok()?;
    let dir: String = key.get_value("InstallLocation").ok()?;
    let version: String = key.get_value("DisplayVersion").unwrap_or_default();
    Some(Existing { dir: dir.trim_matches('"').to_string(), version })
}

#[cfg(not(windows))]
pub fn existing_install() -> Option<Existing> {
    None
}

/// Writes the embedded installer to a temp folder, runs it silently, reports progress, then
/// applies the shortcut choices. `report(pct, label)` is called every ~150 ms.
#[cfg(windows)]
pub fn run(payload: &[u8], expected: u64, opts: &Options, report: &mut dyn FnMut(f64, &str)) -> Outcome {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let fail = |msg: String, code: Option<i32>| Outcome { ok: false, exit_code: code, exe: String::new(), message: msg };
    if payload.is_empty() {
        return fail("This setup was built without the Prospack installer inside it.".into(), None);
    }
    let tmp = std::env::temp_dir().join(format!("prospack-setup-{}", std::process::id()));
    if let Err(e) = std::fs::create_dir_all(&tmp) {
        return fail(format!("Couldn't create a temporary folder: {e}"), None);
    }
    let installer = tmp.join("prospack-installer.exe");
    if let Err(e) = std::fs::write(&installer, payload) {
        return fail(format!("Couldn't write the installer: {e}"), None);
    }
    report(0.0, "Preparing");
    let before = dir_size(&opts.dir);
    let mut child = match std::process::Command::new(&installer)
        .args(nsis_args(&opts.dir))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return fail(format!("Couldn't start the installer: {e}"), None),
    };
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                let written = dir_size(&opts.dir).saturating_sub(before);
                let pct = progress(written, expected).max(time_progress(started.elapsed().as_secs_f64(), expected));
                report(pct, phase_label(pct));
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            Err(e) => return fail(format!("Lost track of the installer: {e}"), None),
        }
    };
    let _ = std::fs::remove_dir_all(&tmp);
    let code = status.code();
    if !status.success() {
        return fail(
            format!("The installer stopped with code {}. Close Prospack if it's running and try again.", code.unwrap_or(-1)),
            code,
        );
    }
    report(99.0, "Applying your choices");
    let exe = opts.dir.join("prospack.exe");
    if let Some(programs) = programs_dir() {
        if !opts.start_menu {
            for l in start_menu_links(&programs) {
                let _ = std::fs::remove_file(l);
            }
        }
    }
    if let Some(desktop) = desktop_dir() {
        let link = desktop.join("Prospack.lnk");
        if opts.desktop {
            if !link.exists() {
                if let Ok(l) = mslnk::ShellLink::new(&exe) {
                    let _ = l.create_lnk(&link);
                }
            }
        } else {
            let _ = std::fs::remove_file(link);
        }
    }
    report(100.0, "Done");
    Outcome { ok: true, exit_code: code, exe: exe.to_string_lossy().into_owned(), message: String::new() }
}

#[cfg(windows)]
fn programs_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs"))
}

#[cfg(windows)]
fn desktop_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Desktop"))
}

#[cfg(not(windows))]
pub fn run(_payload: &[u8], _expected: u64, _opts: &Options, _report: &mut dyn FnMut(f64, &str)) -> Outcome {
    Outcome { ok: false, exit_code: None, exe: String::new(), message: "Prospack Setup only runs on Windows.".into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d_flag_is_last_and_unquoted() {
        let a = nsis_args(Path::new(r"C:\Program Files\Prospack"));
        assert_eq!(a[0], "/S");
        assert_eq!(a.last().unwrap(), r"/D=C:\Program Files\Prospack");
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn progress_never_reaches_100_before_exit() {
        assert_eq!(progress(10, 10), 99.0);
        assert_eq!(progress(20, 10), 99.0);
        assert_eq!(progress(5, 10), 50.0);
        assert_eq!(progress(0, 0), 0.0);
    }

    #[test]
    fn time_estimate_moves_but_never_finishes() {
        assert_eq!(time_progress(2.0, 0), 50.0);
        assert_eq!(time_progress(1000.0, 1), 95.0);
        assert!(time_progress(1.0, 400_000_000) < 25.0);
    }

    #[test]
    fn phases_follow_progress() {
        assert_eq!(phase_label(10.0), "Extracting Prospack");
        assert_eq!(phase_label(80.0), "Installing the compression engine");
        assert_eq!(phase_label(95.0), "Creating shortcuts");
        assert_eq!(phase_label(98.0), "Registering uninstaller");
    }

    #[test]
    fn dir_size_counts_nested_files() {
        let t = std::env::temp_dir().join(format!("pp-size-{}", std::process::id()));
        std::fs::create_dir_all(t.join("a/b")).unwrap();
        std::fs::write(t.join("x"), [0u8; 10]).unwrap();
        std::fs::write(t.join("a/b/y"), [0u8; 5]).unwrap();
        assert_eq!(dir_size(&t), 15);
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn start_menu_link_candidates() {
        let p = Path::new("P");
        let l = start_menu_links(p);
        assert!(l[0].ends_with("Prospack.lnk") && l[1].ends_with("Prospack/Prospack.lnk"));
    }
}
