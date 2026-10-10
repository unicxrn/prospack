<script>
  import { fade } from "svelte/transition";
  import Sparkline from "./Sparkline.svelte";
  import { gb, rate, clock, n, STAGE_NAMES } from "./format.js";

  let { job, ahead, onCancel, onResume, onRemove, onReveal } = $props();

  const stageKey = $derived(job.stages[job.stage]?.key);
  const saved = $derived(job.imageBytes ? job.dump.totalBytes - job.imageBytes : null);
  const savedPct = $derived(saved != null ? Math.round((saved / job.dump.totalBytes) * 100) : null);
  const r = $derived(rate(job.rate));
  const rateSource = { compress: "MkPFS · zlib", verify: "Decompress + compare", upload: "FTP", "": "" };

  // Smoothly animated big number.
  let shown = $state(0);
  $effect(() => {
    const target = job.status === "done" ? 100 : job.pct;
    const from = shown;
    if (Math.abs(target - from) < 0.05) { shown = target; return; }
    const t0 = performance.now();
    let raf;
    const step = (t) => {
      const k = Math.min(1, (t - t0) / 380);
      shown = from + (target - from) * (1 - Math.pow(1 - k, 3));
      if (k < 1) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  });

  const description = $derived.by(() => {
    switch (stageKey) {
      case "compress": return `Packing ${n(job.dump.fileCount)} files into one compressed image on this PC.`;
      case "verify": return `Checking every file in the image against the original dump.`;
      case "upload": return `Sending one file to the PS5. If it's interrupted, Retry picks up where it stopped.`;
      default: return `Telling ShadowMountPlus about the new image and waiting for it to mount.`;
    }
  });
</script>

<section class="focus" style:--accent={job.dump.accent}>
  {#if job.status === "running"}
    <div class="left">
      <div class="kicker">Step {job.stage + 1} of 4 · {STAGE_NAMES[stageKey]}</div>
      {#if stageKey === "mount"}
        <div class="big num">{clock(job.stageSecs)}<small>of 5:00</small></div>
      {:else}
        <div class="big num">{shown.toFixed(1)}<small>%</small></div>
      {/if}
      <p class="desc">{description}</p>
      <div class="bigbar"><i style:width="{job.pct}%"></i></div>
      <dl class="stats">
        <div><dt>Elapsed</dt><dd class="num">{clock(job.stageSecs)}</dd></div>
        <div><dt>Remaining</dt><dd class="num">{clock(job.etaSecs)}</dd></div>
        {#if stageKey === "verify"}
          <div><dt>Files checked</dt><dd class="num">{n(job.filesDone)}</dd></div>
          <div><dt>Mismatches</dt><dd class="num" class:ok={!job.mismatches} class:err={job.mismatches}>{job.mismatches}</dd></div>
        {:else if stageKey === "upload"}
          <div><dt>Sent</dt><dd class="num">{gb(job.doneBytes)}</dd></div>
          <div><dt>Image</dt><dd class="num">{gb(job.totalBytes)}</dd></div>
        {:else if stageKey === "compress"}
          <div><dt>Processed</dt><dd class="num">{gb(job.doneBytes)}</dd></div>
          <div><dt>Source</dt><dd class="num">{gb(job.dump.totalBytes)}</dd></div>
        {:else}
          <div><dt>Image</dt><dd class="num">{gb(job.imageBytes)}</dd></div>
          <div><dt>Saved</dt><dd class="num">{gb(saved)}</dd></div>
        {/if}
      </dl>
    </div>
    <div class="right">
      <div class="kicker">Throughput</div>
      <div class="rate">
        <div class="v num">{r.value}<small>{r.unit}</small></div>
        <span class="mono src">{rateSource[job.rateLabel] ?? ""}</span>
      </div>
      <div class="spark">
        {#if job.rateLabel}
          <Sparkline values={job.rateHistory} color={job.dump.accent} />
        {:else}
          <div class="idle">
            <span class="pulse"></span>
            <span class="mono">Checking /user/app/{job.dump.titleId}/mount_img.lnk every 3 s</span>
          </div>
        {/if}
      </div>
      <div class="actions">
        <button class="btn" onclick={onCancel}>Cancel</button>
        {#if saved != null && saved > 0}<span class="saved num">Saving {gb(saved)} · {savedPct}%</span>{/if}
      </div>
    </div>
  {:else}
    <div class="summary" in:fade={{ duration: 250 }}>
      {#if job.status === "queued"}
        <div class="kicker">Queued</div>
        <h2>{ahead ? `Waiting for ${ahead} ${ahead === 1 ? "game" : "games"} ahead` : "Starting…"}</h2>
        <p class="desc">{n(job.dump.fileCount)} files, {gb(job.dump.totalBytes)}. Packer handles one game at a time.</p>
        <div class="actions"><button class="btn ghost" onclick={onRemove}>Remove from queue</button></div>
      {:else if job.status === "ready"}
        <div class="kicker ok">Verified</div>
        <h2>Ready to upload</h2>
        <p class="desc">
          The image matches the dump file for file. It's {gb(job.imageBytes)}, {savedPct}% smaller than the original.
        </p>
        <div class="actions">
          <button class="btn accent" onclick={onResume}>Upload to PS5</button>
          <button class="btn ghost" onclick={onReveal}>Show image</button>
        </div>
      {:else if job.status === "done"}
        <div class="kicker ok">{job.stages[3].state === "warn" ? "Uploaded" : "Done"}</div>
        <h2>{job.stages[3].state === "warn" ? "On the PS5, not mounted yet" : "Ready to play"}</h2>
        <dl class="stats wide">
          <div><dt>Original</dt><dd class="num">{gb(job.dump.totalBytes)}</dd></div>
          <div><dt>Image</dt><dd class="num">{gb(job.imageBytes)}</dd></div>
          <div><dt>Saved</dt><dd class="num ok">{gb(saved)} · {savedPct}%</dd></div>
          <div><dt>On PS5</dt><dd class="mono path">{job.remotePath}</dd></div>
        </dl>
        {#if job.stages[3].state === "warn"}
          <p class="desc">
            The image is on the PS5 and listed in ShadowMountPlus's manual.lst, but the mount wasn't confirmed in time.
            It often appears a little later; check the game list on the PS5.
          </p>
        {/if}
        <div class="actions">
          {#if job.stages[3].state === "warn"}<button class="btn primary" onclick={onResume}>Check again</button>{/if}
          <button class="btn" onclick={onReveal}>Show image</button>
          <button class="btn ghost" onclick={onRemove}>Clear from list</button>
        </div>
      {:else if job.status === "failed"}
        <div class="kicker err">Failed at {STAGE_NAMES[stageKey]}</div>
        <h2>{stageKey === "verify" ? "The image doesn't match the dump" : "Something went wrong"}</h2>
        <p class="desc error">{job.error}</p>
        {#if job.problems.length}
          <ul class="problems mono">
            {#each job.problems.slice(0, 8) as p}
              <li><span class="pk">{p.kind}</span><span class="pp">{p.path}</span><span class="pd">{p.detail}</span></li>
            {/each}
            {#if job.problems.length > 8}<li class="more">and {job.problems.length - 8} more in the activity log</li>{/if}
          </ul>
        {/if}
        <div class="actions">
          <button class="btn primary" onclick={onResume}>{stageKey === "verify" ? "Compress again" : "Retry"}</button>
          <button class="btn ghost" onclick={onRemove}>Remove</button>
        </div>
      {:else}
        <div class="kicker">Cancelled</div>
        <h2>Stopped at {STAGE_NAMES[stageKey]}</h2>
        <p class="desc">Finished steps are kept, so resuming continues from {STAGE_NAMES[stageKey].toLowerCase()}.</p>
        <div class="actions">
          <button class="btn primary" onclick={onResume}>Resume</button>
          <button class="btn ghost" onclick={onRemove}>Remove</button>
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .focus {
    display: grid; grid-template-columns: 1.15fr 1fr; border: 1px solid var(--line); border-radius: 14px;
    overflow: hidden;
  }
  .focus:has(.left) { min-height: 300px;
  }
  .left, .right, .summary { padding: 26px 28px; }
  .left { border-right: 1px solid var(--line); }
  .summary { grid-column: 1 / -1; max-width: 760px; }
  .big { font-weight: 300; font-size: clamp(56px, 7vw, 88px); letter-spacing: -0.05em; line-height: 1; margin: 14px 0 6px; }
  .big small { font-size: 0.38em; color: var(--muted); letter-spacing: -0.02em; margin-left: 4px; font-weight: 400; }
  .desc { color: var(--muted); max-width: 52ch; }
  .desc.error { color: var(--text); user-select: text; }
  .bigbar { height: 6px; border-radius: 6px; background: var(--line); margin-top: 22px; overflow: hidden; position: relative; }
  .bigbar i { position: absolute; inset: 0 auto 0 0; background: var(--accent); border-radius: 6px; transition: width 0.5s var(--ease); }
  .stats { display: grid; grid-template-columns: repeat(4, 1fr); margin-top: 26px; gap: 0 12px; }
  .stats.wide { grid-template-columns: repeat(3, auto) 1fr; gap: 0 36px; margin: 22px 0 6px; }
  dt { font-size: 12px; color: var(--muted); }
  dd { font-size: 15px; font-weight: 500; margin-top: 4px; }
  dd.path { font-size: 12.5px; font-weight: 400; user-select: text; word-break: break-all; }
  .ok { color: var(--ok); }
  .err { color: var(--err); }
  .kicker.ok { color: var(--ok); }
  .kicker.err { color: var(--err); }
  h2 { font-weight: 500; font-size: 28px; letter-spacing: -0.03em; margin: 12px 0 8px; }

  .right { display: flex; flex-direction: column; }
  .rate { display: flex; justify-content: space-between; align-items: baseline; margin-top: 10px; }
  .rate .v { font-size: 26px; letter-spacing: -0.03em; }
  .rate small { font-size: 13px; color: var(--muted); margin-left: 4px; }
  .src { color: var(--faint); }
  .spark { flex: 1; min-height: 110px; margin: 14px -4px 0; position: relative; }
  .idle { position: absolute; inset: 0; display: flex; flex-direction: column; gap: 18px; align-items: center; justify-content: center; color: var(--faint); text-align: center; }
  .pulse { width: 10px; height: 10px; border-radius: 50%; background: var(--accent); position: relative; }
  .pulse::before, .pulse::after { content: ""; position: absolute; inset: 0; border-radius: 50%; border: 1px solid var(--accent); animation: ring 2.4s var(--ease) infinite; }
  .pulse::after { animation-delay: 1.2s; }
  @keyframes ring { from { transform: scale(1); opacity: 0.8; } to { transform: scale(5); opacity: 0; } }
  .actions { display: flex; gap: 8px; align-items: center; margin-top: 22px; }
  .saved { margin-left: auto; color: var(--muted); font-size: 12.5px; }

  .problems { list-style: none; margin-top: 16px; border: 1px solid var(--line); border-radius: 10px; overflow: hidden; }
  .problems li { display: grid; grid-template-columns: 70px 1fr auto; gap: 12px; padding: 7px 12px; color: var(--muted); }
  .problems li + li { border-top: 1px solid var(--line); }
  .pk { color: var(--err); }
  .pp { color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; user-select: text; }
  .more { display: block !important; color: var(--faint); }

  @media (max-width: 1100px) {
    .focus { grid-template-columns: 1fr; }
    .left { border-right: 0; border-bottom: 1px solid var(--line); }
  }
  @media (max-width: 700px) {
    .stats, .stats.wide { grid-template-columns: 1fr 1fr; gap: 14px 12px; }
    .left, .right, .summary { padding: 20px; }
  }
</style>
