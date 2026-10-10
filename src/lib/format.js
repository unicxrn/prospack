export function gb(bytes) {
  if (bytes == null) return "unknown";
  const g = bytes / 1e9;
  if (g >= 100) return `${g.toFixed(0)} GB`;
  if (g >= 1) return `${g.toFixed(1)} GB`;
  return `${(bytes / 1e6).toFixed(0)} MB`;
}

export function rate(bps) {
  if (!bps) return { value: "0", unit: "MB/s" };
  if (bps >= 1e9) return { value: (bps / 1e9).toFixed(2), unit: "GB/s" };
  return { value: (bps / 1e6).toFixed(bps >= 1e8 ? 0 : 1), unit: "MB/s" };
}

export function clock(secs) {
  if (secs == null) return "--:--";
  secs = Math.max(0, Math.round(secs));
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  return h ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}` : `${m}:${String(s).padStart(2, "0")}`;
}

export const n = (x) => (x ?? 0).toLocaleString("en-US");

export function time(unix) {
  return new Date(unix * 1000).toTimeString().slice(0, 8);
}

export const STAGE_NAMES = { compress: "Compress", verify: "Verify", upload: "Upload", mount: "Mount" };
