#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod install;

use std::path::PathBuf;
use tauri::Emitter;

/// The real Prospack NSIS installer, copied in by scripts/build-setup.ps1 before building.
const PAYLOAD: &[u8] = include_bytes!("../payload/prospack-setup.exe");
/// Installed size of Prospack in bytes, for the progress bar.
const EXPECTED: &str = include_str!("../payload/size.txt");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn expected() -> u64 {
    EXPECTED.trim().parse().unwrap_or(0)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    version: &'static str,
    default_dir: String,
    existing: Option<install::Existing>,
}

#[tauri::command]
fn info() -> Info {
    Info {
        version: VERSION,
        default_dir: install::default_dir().to_string_lossy().into_owned(),
        existing: install::existing_install(),
    }
}

#[tauri::command]
async fn install(handle: tauri::AppHandle, dir: String, start_menu: bool, desktop: bool) -> Result<install::Outcome, String> {
    let opts = install::Options { dir: PathBuf::from(dir), start_menu, desktop };
    tauri::async_runtime::spawn_blocking(move || {
        install::run(PAYLOAD, expected(), &opts, &mut |pct, label| {
            let _ = handle.emit("progress", serde_json::json!({ "pct": pct, "label": label }));
        })
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn launch(handle: tauri::AppHandle, exe: String) -> Result<(), String> {
    std::process::Command::new(&exe).spawn().map_err(|e| format!("Couldn't open Prospack: {e}"))?;
    handle.exit(0);
    Ok(())
}

/// `prospack-setup --unattended <dir> [--no-desktop] [--no-start-menu]` installs without UI (CI).
fn unattended(args: &[String]) -> Option<i32> {
    let i = args.iter().position(|a| a == "--unattended")?;
    let dir = args.get(i + 1).map(PathBuf::from).unwrap_or_else(install::default_dir);
    let opts = install::Options {
        dir,
        start_menu: !args.iter().any(|a| a == "--no-start-menu"),
        desktop: !args.iter().any(|a| a == "--no-desktop"),
    };
    let out = install::run(PAYLOAD, expected(), &opts, &mut |pct, label| eprintln!("{pct:5.1}% {label}"));
    if !out.ok {
        eprintln!("{}", out.message);
    }
    Some(if out.ok { 0 } else { out.exit_code.unwrap_or(1) })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = unattended(&args) {
        std::process::exit(code);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![info, install, launch])
        .run(tauri::generate_context!())
        .expect("error while running Prospack Setup");
}
