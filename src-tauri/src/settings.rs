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
    /// Delete the local image once it's confirmed on the PS5.
    pub delete_local_after_upload: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ps5_host: "192.168.0.50".into(),
            ftp_port: 2121,
            web_port: 8888,
            remote_dir: "/data/homebrew".into(),
            output_dir: default_output_dir().to_string_lossy().into_owned(),
            compression_level: 7,
            cpu_count: 0,
            verify_workers: 0,
            auto_upload: true,
            write_smp_hints: true,
            delete_local_after_upload: false,
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn default_output_dir() -> PathBuf {
    home().join("PS5 Images")
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("ps5-packer")
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"))
        .join("ps5-packer")
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
