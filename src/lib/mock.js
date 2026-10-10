// Simulated backend for browser previews (npm run dev without Tauri). Nothing real happens.

const listeners = {};
const emit = (name, payload) => (listeners[name] || []).forEach((fn) => fn(payload));
const now = () => Math.floor(Date.now() / 1000);
const STAGES = ["compress", "verify", "upload", "mount"];

let settings = {
  ps5Host: "192.168.0.50", ftpPort: 2121, webPort: 8888, remoteDir: "/data/homebrew",
  outputDir: "/home/you/PS5 Images", compressionLevel: 7, cpuCount: 0, verifyWorkers: 0,
  autoUpload: true, writeSmpHints: true, markGcValidated: true, deleteLocalAfterUpload: false,
};

const sample = {
  path: "/games/PPSA01234-app", titleId: "PPSA01234", titleName: "Sample Game",
  version: "01.019.000", fileCount: 166733, totalBytes: 159841147026, apr: true, hasAmprIndex: true, accent: "#3d7bff",
};
const flag = {
  path: "/games/PPSA05678-app0", titleId: "PPSA05678", titleName: "Another Game",
  version: "01.000.000", fileCount: 299, totalBytes: 130982284503, apr: true, hasAmprIndex: true, accent: "#c9a46a",
};

const blankStages = () => STAGES.map((key) => ({ key, state: "pending", meta: "", pct: 0, secs: 0 }));
const log = (text, tone = "") => ({ t: now(), text, tone });

let jobs = [
  {
    id: 1, dump: sample, status: "running", stage: 1, stages: blankStages(), pct: 0, doneBytes: 0, totalBytes: 0,
    filesDone: 0, filesTotal: 166733, rate: 0, rateHistory: [], rateLabel: "verify", etaSecs: null, stageSecs: 0,
    imagePath: "/home/you/PS5 Images/PPSA01234.ffpfsc", imageBytes: 91_700_000_000, remotePath: null, mismatches: 0,
    problems: [], warnings: [], error: null,
    log: [
      log("Added " + sample.path, "hl"), log("Compress started", "hl"),
      log("ampr_emu.index valid and present; skipping generation"),
      log("Inner image: PPSA01234.exfat (159.84 GB)"), log("Compressed to 91.7 GB (43% smaller) in 3m 41s", "ok"),
      log("Verify started", "hl"), log("Comparing every file in the image with the dump, byte for byte"),
    ],
  },
  {
    id: 2, dump: flag, status: "done", stage: 3, stages: blankStages(), pct: 100, doneBytes: 0, totalBytes: 0,
    filesDone: 0, filesTotal: 0, rate: 0, rateHistory: [], rateLabel: "", etaSecs: null, stageSecs: 0,
    imagePath: "/home/you/PS5 Images/PPSA05678.ffpfsc", imageBytes: 71_733_805_056,
    remotePath: "/data/homebrew/PPSA05678.ffpfsc", mismatches: 0, problems: [], warnings: [], error: null,
    log: [log("Finished", "ok")],
  },
];
jobs[0].stages[0] = { key: "compress", state: "done", meta: "160 GB → 91.7 GB · 3m 41s", pct: 100, secs: 221 };
jobs[0].stages[1].state = "live";
jobs[1].stages = [
  { key: "compress", state: "done", meta: "131 GB → 71.7 GB · 2m 58s", pct: 100, secs: 178 },
  { key: "verify", state: "done", meta: "299 files · all match · 1m 12s", pct: 100, secs: 72 },
  { key: "upload", state: "done", meta: "71.7 GB · 13m 02s", pct: 100, secs: 782 },
  { key: "mount", state: "done", meta: "Mounted by ShadowMountPlus", pct: 100, secs: 9 },
];

function tick() {
  const j = jobs.find((x) => x.status === "running");
  if (j) {
    j.stageSecs += 0.25;
    if (j.stage === 1) {
      const total = j.dump.totalBytes;
      j.totalBytes = total;
      const rate = 1.85e9 + Math.sin(Date.now() / 900) * 1.2e8 + Math.random() * 6e7;
      j.doneBytes = Math.min(total, j.doneBytes + rate * 0.25 * 18);
      j.rate = rate;
      j.pct = (j.doneBytes / total) * 100;
      j.filesDone = Math.round(j.filesTotal * (j.pct / 100));
      j.etaSecs = Math.round((total - j.doneBytes) / rate / 18);
      j.stages[1].meta = `${j.filesDone.toLocaleString("en-US")} of ${j.filesTotal.toLocaleString("en-US")} files · 0 mismatches`;
      if (Math.random() < 0.08) j.log.push(log(`ok data/prein/bg/w${(Math.random() * 9) | 0}_tile_${(Math.random() * 999) | 0}.gnfp`));
      if (j.pct >= 100) {
        Object.assign(j.stages[1], { state: "done", pct: 100, meta: "166,733 files · all match · 1m 31s" });
        j.log.push(log("Verify passed: 166,733 files, 0 mismatches", "ok"), log("Upload started", "hl"));
        Object.assign(j, { stage: 2, pct: 0, doneBytes: 0, rate: 0, rateHistory: [], rateLabel: "upload", stageSecs: 0 });
        j.stages[2].state = "live";
      }
    } else if (j.stage === 2) {
      const total = j.imageBytes;
      j.totalBytes = total;
      const rate = 92e6 + Math.random() * 14e6;
      j.doneBytes = Math.min(total, j.doneBytes + rate * 0.25 * 60);
      j.rate = rate;
      j.pct = (j.doneBytes / total) * 100;
      j.etaSecs = Math.round((total - j.doneBytes) / rate / 60);
      j.stages[2].meta = `${(j.doneBytes / 1e9).toFixed(1)} GB of ${(total / 1e9).toFixed(1)} GB`;
      if (j.pct >= 100) {
        Object.assign(j.stages[2], { state: "done", pct: 100, meta: "91.7 GB · 16m 40s" });
        Object.assign(j, { stage: 3, pct: 0, rateHistory: [], rateLabel: "", stageSecs: 0, remotePath: "/data/homebrew/PPSA01234.ffpfsc" });
        j.stages[3].state = "live";
        j.log.push(log("Uploaded 91.7 GB", "ok"), log("Mount started", "hl"), log("ShadowMountPlus autotune.ini: image_sector=PPSA01234.ffpfsc:65536"));
      }
    } else if (j.stage === 3) {
      j.pct = Math.min(100, j.pct + 2.5);
      j.stages[3].meta = `Waiting for ShadowMountPlus · ${Math.round(j.stageSecs)}s`;
      if (j.pct >= 100) {
        Object.assign(j.stages[3], { state: "done", pct: 100, meta: "Mounted by ShadowMountPlus" });
        j.status = "done";
        j.log.push(log("ShadowMountPlus mounted the image. It's ready to play.", "ok"));
      }
    }
    if (Math.round(j.stageSecs * 4) % 4 === 0 && j.rate) {
      j.rateHistory.push(j.rate);
      if (j.rateHistory.length > 90) j.rateHistory.shift();
    }
    j.stages[j.stage].pct = j.pct;
    emit("jobs", structuredClone(jobs));
  }
}
setInterval(tick, 250);

const delay = (ms) => new Promise((r) => setTimeout(r, ms));

export const mock = {
  getSettings: async () => structuredClone(settings),
  saveSettings: async (v) => { settings = v; },
  engineStatus: async () => ({ python: "3.14.7", mkpfs: "1.1.0", ready: true, dir: "~/.local/share/ps5-packer/engine" }),
  installEngine: async () => { for (const l of ["Creating Python environment", "Installing MkPFS 1.1.0"]) { emit("engine-log", l); await delay(500); } return mock.engineStatus(); },
  inspectDump: async () => { throw new Error("preview"); },
  addJob: async () => { throw "Adding dumps needs the desktop app (this is a browser preview)"; },
  jobArt: async (path) => path.includes("SampleGame") ? { icon: "/mock/icon.jpg", hero: "/mock/pic0.jpg" } : { icon: null, hero: null },
  listJobs: async () => structuredClone(jobs),
  cancelJob: async (id) => { const j = jobs.find((x) => x.id === id); if (j) { j.status = "cancelled"; j.stages[j.stage].state = "pending"; } emit("jobs", structuredClone(jobs)); },
  removeJob: async (id) => { jobs = jobs.filter((x) => x.id !== id); emit("jobs", structuredClone(jobs)); },
  resumeJob: async (id) => { const j = jobs.find((x) => x.id === id); if (j) { j.status = "running"; j.stages[j.stage].state = "live"; } emit("jobs", structuredClone(jobs)); },
  ps5Status: async () => ({ online: true, latencyMs: 4, error: null, freeBytes: 631_629_479_936, totalBytes: 937_046_507_520, shadowmount: true }),
  on: async (name, fn) => { (listeners[name] ||= []).push(fn); return () => {}; },
  pickFolder: async () => null,
  reveal: async () => {},
  onDrop: async () => () => {},
  window: { minimize: async () => {}, toggleMaximize: async () => {}, close: async () => {} },
};
