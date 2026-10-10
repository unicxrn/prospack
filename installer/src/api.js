// Tauri commands, or a simulated installer when previewed in a browser.
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function tauri() {
  const [core, event, dialog, win] = await Promise.all([
    import("@tauri-apps/api/core"),
    import("@tauri-apps/api/event"),
    import("@tauri-apps/plugin-dialog"),
    import("@tauri-apps/api/window"),
  ]);
  return { core, event, dialog, win };
}

const mockListeners = [];
export const api = isTauri
  ? {
      info: async () => (await tauri()).core.invoke("info"),
      install: async (dir, startMenu, desktop) => (await tauri()).core.invoke("install", { dir, startMenu, desktop }),
      launch: async (exe) => (await tauri()).core.invoke("launch", { exe }),
      onProgress: async (fn) => (await tauri()).event.listen("progress", (e) => fn(e.payload)),
      pickDir: async () => (await tauri()).dialog.open({ directory: true, title: "Install Prospack to…" }),
      close: async () => (await tauri()).win.getCurrentWindow().close(),
    }
  : {
      info: async () => ({
        version: "0.1.0",
        defaultDir: "C:\\Users\\you\\AppData\\Local\\Programs\\Prospack",
        existing: new URLSearchParams(location.search).has("update") ? { dir: "C:\\Users\\you\\AppData\\Local\\Programs\\Prospack", version: "0.0.9" } : null,
      }),
      install: async () => {
        const labels = ["Extracting Prospack", "Installing the compression engine", "Creating shortcuts", "Registering uninstaller"];
        for (let p = 0; p <= 100; p += 2 + Math.random() * 3) {
          const pct = Math.min(99, p);
          mockListeners.forEach((f) => f({ pct, label: labels[Math.min(3, Math.floor(pct / 26))] }));
          await new Promise((r) => setTimeout(r, 70));
        }
        mockListeners.forEach((f) => f({ pct: 100, label: "Done" }));
        if (new URLSearchParams(location.search).has("fail")) return { ok: false, exitCode: 2, exe: "", message: "The installer stopped with code 2. Close Prospack if it's running and try again." };
        return { ok: true, exitCode: 0, exe: "prospack.exe", message: "" };
      },
      launch: async () => {},
      onProgress: async (fn) => { mockListeners.push(fn); return () => {}; },
      pickDir: async () => null,
      close: async () => {},
    };
