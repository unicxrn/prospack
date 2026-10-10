use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

/// Carries settings and backups over from the app's earlier name ("Packer", ps5-packer dirs).
/// Returns true when settings were written. Never touches the old folders.
pub fn migrate_from(old_cfg: &Path, old_data: &Path, new_cfg: &Path, new_data: &Path) -> Result<bool, String> {
    let new_settings = new_cfg.join("settings.json");
    let mut wrote = false;
    if !new_settings.exists() {
        let old_settings = old_cfg.join("settings.json");
        let migrated = if let Ok(raw) = std::fs::read_to_string(&old_settings) {
            // They configured the old app on purpose, so the wizard isn't needed.
            let mut s: Settings = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            s.setup_done = true;
            Some(s)
        } else if old_data.is_dir() {
            // Used the old app with its built-in defaults: keep those, but show the wizard.
            let old_images = paths::home().join("PS5 Images");
            Some(Settings {
                ps5_host: "192.168.0.50".into(),
                output_dir: if old_images.is_dir() {
                    old_images.to_string_lossy().into_owned()
                } else {
                    default_output_dir().to_string_lossy().into_owned()
                },
                ..Settings::default()
            })
        } else {
            None
        };
        if let Some(s) = migrated {
            std::fs::create_dir_all(new_cfg).map_err(|e| e.to_string())?;
            std::fs::write(&new_settings, serde_json::to_vec_pretty(&s).unwrap()).map_err(|e| e.to_string())?;
            wrote = true;
        }
    }
    let (old_b, new_b) = (old_data.join("backups"), new_data.join("backups"));
    if old_b.is_dir() && !new_b.exists() {
        std::fs::create_dir_all(&new_b).map_err(|e| e.to_string())?;
        for e in std::fs::read_dir(&old_b).map_err(|e| e.to_string())?.flatten() {
            let _ = std::fs::copy(e.path(), new_b.join(e.file_name()));
        }
    }
    Ok(wrote)
}

pub fn migrate_legacy() {
    if let Err(e) = migrate_from(&paths::legacy_config_dir(), &paths::legacy_data_dir(), &config_dir(), &data_dir()) {
        eprintln!("settings migration failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_old_ps5_packer_settings_once() {
        let t = tempfile::tempdir().unwrap();
        let (old_cfg, old_data) = (t.path().join("old/cfg"), t.path().join("old/data"));
        std::fs::create_dir_all(old_data.join("backups")).unwrap();
        std::fs::write(old_data.join("backups/1-config.ini"), "x").unwrap();
        std::fs::create_dir_all(&old_cfg).unwrap();
        std::fs::write(old_cfg.join("settings.json"), r#"{"ps5Host":"10.0.0.5","ftpPort":2121}"#).unwrap();
        let (new_cfg, new_data) = (t.path().join("new/cfg"), t.path().join("new/data"));
        assert!(migrate_from(&old_cfg, &old_data, &new_cfg, &new_data).unwrap());
        let s: Settings = serde_json::from_str(&std::fs::read_to_string(new_cfg.join("settings.json")).unwrap()).unwrap();
        assert_eq!(s.ps5_host, "10.0.0.5");
        assert!(s.setup_done, "people who configured Packer skip the wizard");
        assert!(new_data.join("backups/1-config.ini").is_file());
        assert!(!migrate_from(&old_cfg, &old_data, &new_cfg, &new_data).unwrap(), "second run is a no-op");
    }

    #[test]
    fn old_defaults_user_gets_old_host_and_the_wizard() {
        let t = tempfile::tempdir().unwrap();
        let old_data = t.path().join("old/data");
        std::fs::create_dir_all(&old_data).unwrap();
        let new_cfg = t.path().join("new/cfg");
        assert!(migrate_from(&t.path().join("old/cfg"), &old_data, &new_cfg, &t.path().join("new/data")).unwrap());
        let s: Settings = serde_json::from_str(&std::fs::read_to_string(new_cfg.join("settings.json")).unwrap()).unwrap();
        assert_eq!(s.ps5_host, "192.168.0.50");
        assert!(!s.setup_done);
    }

    #[test]
    fn fresh_install_writes_nothing() {
        let t = tempfile::tempdir().unwrap();
        let new_cfg = t.path().join("new/cfg");
        assert!(!migrate_from(&t.path().join("a"), &t.path().join("b"), &new_cfg, &t.path().join("c")).unwrap());
        assert!(!new_cfg.join("settings.json").exists());
    }
}
