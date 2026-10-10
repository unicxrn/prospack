// Thin wrapper over the Tauri backend. Outside Tauri (plain browser) it falls back to a
// simulated backend so the interface can be previewed without touching real files.

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

let real = null;
async function tauri() {
  if (!real) {
    const [core, event, dialog, opener, win, webview] = await Promise.all([
      import("@tauri-apps/api/core"),
      import("@tauri-apps/api/event"),
      import("@tauri-apps/plugin-dialog"),
      import("@tauri-apps/plugin-opener"),
      import("@tauri-apps/api/window"),
      import("@tauri-apps/api/webview"),
    ]);
    real = { core, event, dialog, opener, win, webview };
  }
  return real;
}

const call = async (cmd, args) => (await tauri()).core.invoke(cmd, args);

export const api = isTauri
  ? {
      getSettings: () => call("get_settings"),
      saveSettings: (value) => call("save_settings", { value }),
      engineStatus: () => call("engine_status"),
      installEngine: () => call("install_engine"),
      inspectDump: (path) => call("inspect_dump", { path }),
      addJob: (path) => call("add_job", { path }),
      jobArt: (path, titleId) => call("job_art", { path, titleId }),
      listJobs: () => call("list_jobs"),
      cancelJob: (id) => call("cancel_job", { id }),
      removeJob: (id) => call("remove_job", { id }),
      resumeJob: (id) => call("resume_job", { id }),
      ps5Status: (value) => call("ps5_status", { value: value ?? null }),
      discoverPs5: () => call("discover_ps5"),
      freeSpace: (path) => call("free_space", { path }),
      on: async (name, fn) => (await tauri()).event.listen(name, (e) => fn(e.payload)),
      pickFolder: async (title) => (await tauri()).dialog.open({ directory: true, multiple: false, title }),
      reveal: async (path) => (await tauri()).opener.revealItemInDir(path),
      onDrop: async (fn) => (await tauri()).webview.getCurrentWebview().onDragDropEvent((e) => fn(e.payload)),
      window: {
        minimize: async () => (await tauri()).win.getCurrentWindow().minimize(),
        toggleMaximize: async () => (await tauri()).win.getCurrentWindow().toggleMaximize(),
        close: async () => (await tauri()).win.getCurrentWindow().close(),
      },
    }
  : (await import("./mock.js")).mock;
