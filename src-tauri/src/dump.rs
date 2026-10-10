//! Reads a dump folder. Never writes to it.

use base64::Engine;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Files whose presence marks an APR Emu title (same list Game Compressor checks).
pub const AMPR_MARKERS: [&str; 4] = [
    "fakelib/libSceAmpr.sprx",
    "fakelib/libSceAmpr.prx",
    "sce_module/libSceAmpr.sprx",
    "sce_module/libSceAmpr.prx",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpInfo {
    pub path: String,
    pub title_id: String,
    pub title_name: String,
    pub version: String,
    pub file_count: u64,
    pub total_bytes: u64,
    pub apr: bool,
    pub has_ampr_index: bool,
    /// Hex colour pulled from the icon, used as the UI accent for this game.
    pub accent: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpArt {
    /// data: URLs, so the webview needs no file access.
    pub icon: Option<String>,
    pub hero: Option<String>,
}

pub fn inspect(path: &Path) -> Result<DumpInfo, String> {
    let param_path = path.join("sce_sys/param.json");
    let raw = std::fs::read_to_string(&param_path)
        .map_err(|_| "This folder isn't a PS5 dump: sce_sys/param.json is missing.".to_string())?;
    let param: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("sce_sys/param.json is not valid JSON: {e}"))?;

    let title_id = param["titleId"].as_str().unwrap_or("").trim().to_string();
    if title_id.len() != 9 || !title_id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(format!("Unexpected title ID {title_id:?} in sce_sys/param.json."));
    }
    let loc = &param["localizedParameters"];
    let lang = loc["defaultLanguage"].as_str().unwrap_or("en-US");
    let title_name = loc[lang]["titleName"]
        .as_str()
        .or_else(|| loc["en-US"]["titleName"].as_str())
        .unwrap_or(&title_id)
        .to_string();
    let version = param["contentVersion"].as_str().unwrap_or("").to_string();

    if !path.join("eboot.bin").is_file() {
        return Err("This folder has no eboot.bin, so it doesn't look like a complete game dump.".into());
    }

    let (file_count, total_bytes) = tally(path);
    let apr = AMPR_MARKERS.iter().any(|m| path.join(m).is_file());
    let has_ampr_index = path.join("ampr_emu.index").is_file();
    let accent = icon_path(path)
        .and_then(|p| image::open(p).ok())
        .map(|img| accent_from(&img))
        .unwrap_or_else(|| "#3d7bff".into());

    Ok(DumpInfo {
        path: path.to_string_lossy().into_owned(),
        title_id,
        title_name,
        version,
        file_count,
        total_bytes,
        apr,
        has_ampr_index,
        accent,
    })
}

/// File count and byte total, skipping symlinks (MkPFS refuses them anyway).
pub fn tally(root: &Path) -> (u64, u64) {
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            } else if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() {
                files += 1;
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    (files, bytes)
}

fn icon_path(dump: &Path) -> Option<PathBuf> {
    let p = dump.join("sce_sys/icon0.png");
    p.is_file().then_some(p)
}

fn hero_path(dump: &Path) -> Option<PathBuf> {
    ["sce_sys/pic0.png", "sce_sys/pic1.png", "sce_sys/pic2.png"]
        .iter()
        .map(|p| dump.join(p))
        .find(|p| p.is_file())
}

/// Downscaled artwork as data URLs, cached on disk per title so re-adding is instant.
pub fn art(dump: &Path, title_id: &str, cache_root: &Path) -> DumpArt {
    let cache = cache_root.join("art").join(title_id);
    let _ = std::fs::create_dir_all(&cache);
    let icon = cached_jpeg(icon_path(dump), &cache.join("icon.jpg"), 256);
    let hero = cached_jpeg(hero_path(dump), &cache.join("hero.jpg"), 1920);
    DumpArt { icon, hero }
}

fn cached_jpeg(src: Option<PathBuf>, dst: &Path, width: u32) -> Option<String> {
    let src = src?;
    let fresh = match (std::fs::metadata(&src), std::fs::metadata(dst)) {
        (Ok(s), Ok(d)) => d.modified().ok() >= s.modified().ok(),
        _ => false,
    };
    if !fresh {
        let img = image::open(&src).ok()?;
        let img = if img.width() > width {
            img.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };
        let mut out = std::fs::File::create(dst).ok()?;
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 86);
        img.to_rgb8().write_with_encoder(enc).ok()?;
    }
    let bytes = std::fs::read(dst).ok()?;
    Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Picks the most saturated common colour from the icon, nudged to stay readable on near-black.
fn accent_from(img: &image::DynamicImage) -> String {
    let small = img.resize_exact(48, 48, image::imageops::FilterType::Triangle).to_rgb8();
    // 12 hue buckets weighted by saturation * value.
    let mut buckets = [(0f64, 0f64, 0f64, 0f64); 12];
    for p in small.pixels() {
        let (r, g, b) = (p[0] as f64 / 255.0, p[1] as f64 / 255.0, p[2] as f64 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let sat = if max > 0.0 { (max - min) / max } else { 0.0 };
        if sat < 0.25 || max < 0.2 {
            continue;
        }
        let hue = hue_of(r, g, b, max, min);
        let w = sat * max;
        let i = ((hue / 30.0) as usize).min(11);
        buckets[i].0 += r * w;
        buckets[i].1 += g * w;
        buckets[i].2 += b * w;
        buckets[i].3 += w;
    }
    let best = buckets.iter().max_by(|a, b| a.3.total_cmp(&b.3)).copied().unwrap();
    if best.3 == 0.0 {
        return "#3d7bff".into();
    }
    let (mut r, mut g, mut b) = (best.0 / best.3, best.1 / best.3, best.2 / best.3);
    // Lift dark accents so they read on the #0b0b0c background.
    let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if lum < 0.32 {
        let k = 0.32 / lum.max(0.05);
        let k = k.min(2.6);
        r = (r * k).min(1.0);
        g = (g * k).min(1.0);
        b = (b * k).min(1.0);
    }
    format!("#{:02x}{:02x}{:02x}", (r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn hue_of(r: f64, g: f64, b: f64, max: f64, min: f64) -> f64 {
    let d = max - min;
    if d == 0.0 {
        return 0.0;
    }
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h * 60.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dump(dir: &Path) {
        std::fs::create_dir_all(dir.join("sce_sys")).unwrap();
        std::fs::create_dir_all(dir.join("fakelib")).unwrap();
        std::fs::write(
            dir.join("sce_sys/param.json"),
            r#"{"titleId":"PPSA00001","contentVersion":"01.002.000","localizedParameters":{"defaultLanguage":"en-US","en-US":{"titleName":"TEST GAME"}}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("eboot.bin"), vec![1u8; 1000]).unwrap();
        std::fs::write(dir.join("fakelib/libSceAmpr.sprx"), b"x").unwrap();
    }

    #[test]
    fn inspects_a_dump() {
        let t = tempfile::tempdir().unwrap();
        make_dump(t.path());
        let info = inspect(t.path()).unwrap();
        assert_eq!(info.title_id, "PPSA00001");
        assert_eq!(info.title_name, "TEST GAME");
        assert_eq!(info.version, "01.002.000");
        assert_eq!(info.file_count, 3);
        assert!(info.apr);
        assert!(!info.has_ampr_index);
    }

    #[test]
    fn rejects_non_dumps() {
        let t = tempfile::tempdir().unwrap();
        assert!(inspect(t.path()).is_err());
    }

    #[test]
    fn accent_is_lifted_for_dark_icons() {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([10, 20, 120])));
        let hex = accent_from(&img);
        let v = u32::from_str_radix(&hex[1..], 16).unwrap();
        assert!(((v >> 16) & 0xff) + ((v >> 8) & 0xff) + (v & 0xff) > 150, "{hex} too dark");
    }
}
