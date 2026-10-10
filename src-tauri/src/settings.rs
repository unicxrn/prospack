use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub ps5_host: String,
    pub ftp_port: u16,
    pub web_port: u16,
    /// Where finished images go on the PS5. ShadowMountPlus scans this by default.
    pub remote_dir: String,
    /// Where images are written on this PC.
    pub output_dir: String,
    pub compression_level: u8,
    /// 0 = let MkPFS decide.
    pub cpu_count: u32,
    /// 0 = half the cores, capped at 8.
    pub verify_workers: u32,
    /// Continue to upload straight after a passed verify.
    pub auto_upload: bool,
    /// Write the image_sector / image_ro hints Game Compressor writes.
    pub write_smp_hints: bool,
    /// Write Game Compressor's validation record so GC shows the game as validated.
    pub mark_gc_validated: bool,
    /// Delete the local image once it's confirmed on the PS5.
    pub delete_local_after_upload: bool,
    /// The first-run wizard has been completed (or skipped).
    pub setup_done: bool,
    /// Look for a new Prospack release at startup and every few hours.
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ps5_host: String::new(),
            ftp_port: 2121,
            web_port: 8888,
            remote_dir: "/data/homebrew".into(),
            output_dir: default_output_dir().to_string_lossy().into_owned(),
            compression_level: 7,
            cpu_count: 0,
            verify_workers: 0,
            auto_upload: true,
            write_smp_hints: true,
            mark_gc_validated: true,
            delete_local_after_upload: false,
            setup_done: false,
            check_updates: true,
        }
    }
}

fn default_output_dir() -> PathBuf {
    paths::home().join("Prospack Images")
}

pub fn config_dir() -> PathBuf {
    paths::config_dir()
}

pub fn data_dir() -> PathBuf {
    paths::data_dir()
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn load() -> Settings {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(s: &Settings) -> Result<(), String> {
    let path = settings_path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s).unwrap()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}
