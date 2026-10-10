//! Where Prospack keeps its config and data. Tauri supplies the platform dirs at startup;
//! tests and CLI runs fall back to $PROSPACK_CONFIG_DIR / $PROSPACK_DATA_DIR.

use std::path::PathBuf;
use std::sync::OnceLock;

static CONFIG: OnceLock<PathBuf> = OnceLock::new();
static DATA: OnceLock<PathBuf> = OnceLock::new();

pub fn init(config: PathBuf, data: PathBuf) {
    let _ = CONFIG.set(config);
    let _ = DATA.set(data);
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("PROSPACK_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| CONFIG.get().cloned())
        .unwrap_or_else(|| home().join(".config/prospack"))
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("PROSPACK_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| DATA.get().cloned())
        .unwrap_or_else(|| home().join(".local/share/prospack"))
}
