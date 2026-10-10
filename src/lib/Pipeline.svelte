<script>
  import { STAGE_NAMES } from "./format.js";
  let { job } = $props();

  const defaults = {
    compress: "MkPFS · exFAT in PFSC",
    verify: "Every file, byte for byte",
    upload: "Single file over FTP",
    mount: "ShadowMountPlus",
  };
</script>

<section class="pipe">
  {#each job.stages as s, i (s.key)}
    <div class="stage {s.state}">
      <div class="top">
        <span class="idx">0{i + 1}</span>
        <span class="state">
          {#if s.state === "done"}
            <svg width="9" height="9" viewBox="0 0 10 10"><path d="M2 5.2l2 2L8 3" stroke="#0b0b0c" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round" /></svg>
          {:else if s.state === "failed"}
            <svg width="8" height="8" viewBox="0 0 10 10"><path d="M2.5 2.5l5 5M7.5 2.5l-5 5" stroke="#0b0b0c" stroke-width="1.8" stroke-linecap="round" /></svg>
          {:else if s.state === "warn"}
            <svg width="8" height="8" viewBox="0 0 10 10"><path d="M5 2v3.6M5 7.6v.1" stroke="#0b0b0c" stroke-width="1.8" stroke-linecap="round" /></svg>
          {/if}
        </span>
      </div>
      <div class="label">{STAGE_NAMES[s.key]}</div>
      <div class="meta num">{s.meta || defaults[s.key]}</div>
      <div class="track"><i style:width="{s.state === 'done' || s.state === 'warn' ? 100 : s.pct}%"></i></div>
    </div>
  {/each}
</section>

<style>
  .pipe {
    display: grid; grid-template-columns: repeat(4, 1fr); border: 1px solid var(--line); border-radius: 14px;
    overflow: hidden; background: var(--surface);
  }
  .stage { padding: 16px 18px 18px; position: relative; transition: background 0.5s var(--ease); min-width: 0; }
  .stage + .stage { border-left: 1px solid var(--line); }
  .top { display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; }
  .idx { font: 500 11px var(--mono); color: var(--faint); }
  .state {
    width: 16px; height: 16px; border-radius: 50%; border: 1.5px solid var(--line-strong);
    display: grid; place-items: center; transition: all 0.4s var(--ease);
  }
  .label { font-weight: 500; font-size: 14px; }
  .meta { color: var(--muted); font-size: 12px; margin-top: 3px; min-height: 17px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .track { height: 2px; background: var(--line); margin-top: 14px; border-radius: 2px; overflow: hidden; position: relative; }
  .track i { position: absolute; inset: 0 auto 0 0; background: var(--text); transition: width 0.5s var(--ease), background 0.4s; }

  .pending .label, .pending .meta { color: var(--faint); }
  .pending .track i { width: 0 !important; }
  .done .state { background: var(--ok); border-color: var(--ok); }
  .done .track i { background: var(--ok); }
  .warn .state { background: var(--warn); border-color: var(--warn); }
  .warn .track i { background: var(--warn); }
  .failed .state { background: var(--err); border-color: var(--err); }
  .failed .track i { background: var(--err); }
  .failed .meta { color: var(--err); }
  .live { background: var(--raised); }
  .live .state { border-color: var(--accent); }
  .live .state::after {
    content: ""; width: 6px; height: 6px; border-radius: 50%; background: var(--accent);
    animation: breathe 1.6s ease-in-out infinite;
  }
  @keyframes breathe { 50% { transform: scale(0.55); opacity: 0.5; } }
  .live .track i { background: var(--accent); }
  .live .track::after {
    content: ""; position: absolute; inset: 0; width: 30%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.55), transparent);
    animation: sweep 2.2s var(--ease) infinite;
  }
  @keyframes sweep { from { transform: translateX(-100%); } to { transform: translateX(400%); } }

  @media (max-width: 1000px) {
    .pipe { grid-template-columns: 1fr 1fr; }
    .stage:nth-child(3) { border-left: 0; }
    .stage:nth-child(n + 3) { border-top: 1px solid var(--line); }
  }
</style>
