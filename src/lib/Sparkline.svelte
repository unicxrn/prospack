<script>
  let { values = [], color = "var(--accent)" } = $props();
  const W = 400, H = 120;
  const uid = Math.random().toString(36).slice(2, 8);

  const path = $derived.by(() => {
    if (values.length < 2) return null;
    // Scale to the 90th percentile so one spike (e.g. a burst at stage start) can't flatten the line.
    const sorted = [...values].sort((a, b) => a - b);
    const max = sorted[Math.floor((sorted.length - 1) * 0.9)] * 1.35 || 1;
    const step = W / (values.length - 1);
    const pts = values.map((v, i) => [i * step, H - Math.min(1, v / max) * H]);
    // Smooth with a simple cardinal spline so the line doesn't look jagged.
    let d = `M${pts[0][0].toFixed(1)},${pts[0][1].toFixed(1)}`;
    for (let i = 0; i < pts.length - 1; i++) {
      const [x0, y0] = pts[Math.max(0, i - 1)];
      const [x1, y1] = pts[i];
      const [x2, y2] = pts[i + 1];
      const [x3, y3] = pts[Math.min(pts.length - 1, i + 2)];
      const c1x = x1 + (x2 - x0) / 6, c1y = y1 + (y2 - y0) / 6;
      const c2x = x2 - (x3 - x1) / 6, c2y = y2 - (y3 - y1) / 6;
      d += ` C${c1x.toFixed(1)},${c1y.toFixed(1)} ${c2x.toFixed(1)},${c2y.toFixed(1)} ${x2.toFixed(1)},${y2.toFixed(1)}`;
    }
    return { line: d, fill: `${d} L${W},${H} L0,${H} Z` };
  });
</script>

<svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" aria-hidden="true">
  <defs>
    <linearGradient id="g{uid}" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color={color} stop-opacity="0.22" />
      <stop offset="1" stop-color={color} stop-opacity="0" />
    </linearGradient>
  </defs>
  {#each [0.25, 0.5, 0.75] as g}
    <line x1="0" x2={W} y1={H * g} y2={H * g} stroke="rgba(255,255,255,.04)" vector-effect="non-scaling-stroke" />
  {/each}
  {#if path}
    <path d={path.fill} fill="url(#g{uid})" />
    <path d={path.line} fill="none" stroke={color} stroke-width="1.5" vector-effect="non-scaling-stroke" />
  {/if}
</svg>

<style>
  svg { width: 100%; height: 100%; display: block; overflow: visible; }
  path { transition: d 0.4s var(--ease); }
</style>
