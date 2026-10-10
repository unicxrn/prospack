mod discover;
mod dump;
mod engine;
mod ftp;
mod paths;
mod pipeline;
mod ps5;
mod settings;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter as _, Manager, State};

pub type Emitter = Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>;

struct App {
    queue: pipeline::Queue,
    engine: Arc<engine::Engine>,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings() -> settings::Settings {
    settings::load()
}

#[tauri::command]
fn save_settings(value: settings::Settings) -> Result<(), String> {
    settings::save(&value)
}

#[tauri::command]
async fn engine_status(app: State<'_, App>) -> Result<engine::EngineStatus, String> {
    let e = app.engine.clone();
    blocking(move || e.status()).await
}

#[tauri::command]
async fn install_engine(app: State<'_, App>, handle: tauri::AppHandle) -> Result<engine::EngineStatus, String> {
    let e = app.engine.clone();
    blocking(move || {
        e.install(&mut |line| {
            let _ = handle.emit("engine-log", line);
        })?;
        Ok(e.status())
    })
    .await?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Inspected {
    info: dump::DumpInfo,
    art: dump::DumpArt,
}

#[tauri::command]
async fn inspect_dump(path: String) -> Result<Inspected, String> {
    blocking(move || {
        let p = PathBuf::from(&path);
        let info = dump::inspect(&p)?;
        let art = dump::art(&p, &info.title_id, &settings::data_dir());
        Ok(Inspected { info, art })
    })
    .await?
}

#[tauri::command]
async fn add_job(app: State<'_, App>, path: String) -> Result<u64, String> {
    let q = app.queue.clone();
    blocking(move || {
        let info = dump::inspect(&PathBuf::from(&path))?;
        q.add(info)
    })
    .await?
}

#[tauri::command]
async fn job_art(path: String, title_id: String) -> Result<dump::DumpArt, String> {
    blocking(move || dump::art(&PathBuf::from(path), &title_id, &settings::data_dir())).await
}

#[tauri::command]
fn list_jobs(app: State<'_, App>) -> Vec<pipeline::Job> {
    app.queue.snapshot()
}

#[tauri::command]
fn cancel_job(app: State<'_, App>, id: u64) {
    app.queue.cancel(id)
}

#[tauri::command]
fn remove_job(app: State<'_, App>, id: u64) -> Result<(), String> {
    app.queue.remove(id)
}

#[tauri::command]
fn resume_job(app: State<'_, App>, id: u64) -> Result<(), String> {
    app.queue.resume(id)
}

#[tauri::command]
async fn ps5_status(value: Option<settings::Settings>) -> Result<ps5::Ps5Status, String> {
    blocking(move || ps5::status(&value.unwrap_or_else(settings::load))).await
}

#[tauri::command]
async fn discover_ps5() -> Result<Vec<discover::Found>, String> {
    blocking(discover::discover).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            paths::init(app.path().app_config_dir()?, app.path().app_data_dir()?);
            settings::migrate_legacy();
            let handle = app.handle().clone();
            let emitter: Emitter = Arc::new(move |event, payload| {
                let _ = handle.emit(event, payload);
            });
            let engine = Arc::new(engine::Engine::new(&settings::data_dir()));
            let queue = pipeline::Queue::new(emitter, engine.clone());
            // Folders passed on the command line (or via "Open with") are queued straight away.
            for arg in std::env::args().skip(1).filter(|a| !a.starts_with('-')) {
                let p = PathBuf::from(&arg);
                if p.is_dir() {
                    if let Err(e) = dump::inspect(&p).and_then(|info| queue.add(info)) {
                        eprintln!("{arg}: {e}");
                    }
                }
            }
            app.manage(App { queue, engine });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            engine_status,
            install_engine,
            inspect_dump,
            add_job,
            job_art,
            list_jobs,
            cancel_job,
            remove_job,
            resume_job,
            ps5_status,
            discover_ps5
        ])
        .run(tauri::generate_context!())
        .expect("error while running Prospack");
}
