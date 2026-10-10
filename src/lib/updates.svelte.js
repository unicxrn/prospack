// Update state shared by the title bar, the update dialog, Help and Settings.
import { api } from "./api.js";

const REPO = "unicxrn/prospack";
const SIX_HOURS = 6 * 60 * 60 * 1000;

export const updates = $state({
  current: null,
  kind: null, // "windows" | "appimage" | "system"
  available: null, // { version, notes, canInstall }
  checking: false,
  lastError: null,
  lastChecked: null,
  // While downloading or installing: { got, total }
  progress: null,
  installing: false,
});

let handle = null; // the updater's Update object, needed to install

/** "0.10.2" > "0.9.9", ignoring a leading "v" and any "-suffix". */
export function newer(a, b) {
  const parts = (v) => String(v).replace(/^v/, "").split("-")[0].split(".").map((n) => parseInt(n, 10) || 0);
  const [x, y] = [parts(a), parts(b)];
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) > (y[i] ?? 0);
  }
  return false;
}

/** A .deb or source build can't replace itself; ask GitHub what the latest release is instead. */
async function latestFromGitHub() {
  const r = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: "application/vnd.github+json" } });
  if (!r.ok) throw new Error(`GitHub answered ${r.status}`);
  const rel = await r.json();
  return { version: rel.tag_name.replace(/^v/, ""), notes: rel.body ?? "" };
}

export async function checkForUpdates() {
  if (updates.checking || updates.installing) return;
  updates.checking = true;
  updates.lastError = null;
  try {
    updates.current ??= await api.appVersion();
    updates.kind ??= await api.installKind();
    if (updates.kind === "system") {
      const rel = await latestFromGitHub();
      updates.available = newer(rel.version, updates.current) ? { ...rel, canInstall: false } : null;
    } else {
      handle = await api.checkUpdate();
      updates.available = handle ? { version: handle.version, notes: handle.body ?? "", canInstall: true } : null;
    }
  } catch (e) {
    updates.lastError = String(e?.message ?? e);
  } finally {
    updates.checking = false;
    updates.lastChecked = Date.now();
  }
}

/** Downloads, installs and restarts. On Windows the installer closes the app itself. */
export async function installUpdate() {
  if (!handle) return;
  updates.installing = true;
  updates.progress = { got: 0, total: 0 };
  try {
    await handle.downloadAndInstall((ev) => {
      if (ev.event === "Started") updates.progress = { got: 0, total: ev.data.contentLength ?? 0 };
      else if (ev.event === "Progress") updates.progress = { ...updates.progress, got: updates.progress.got + ev.data.chunkLength };
    });
    await api.relaunch();
  } catch (e) {
    updates.lastError = String(e?.message ?? e);
    updates.installing = false;
    updates.progress = null;
  }
}

export function releasePage(version) {
  return `https://github.com/${REPO}/releases/tag/v${version}`;
}

/** Checks now and then every six hours while `enabled()` stays true. Returns a stop function. */
export function startAutoCheck(enabled) {
  if (enabled()) checkForUpdates();
  const t = setInterval(() => enabled() && checkForUpdates(), SIX_HOURS);
  return () => clearInterval(t);
}
