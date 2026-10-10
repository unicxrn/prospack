<script>
  import { gb, n } from "./format.js";
  let { job, art } = $props();
</script>

<section class="hero" style:--accent={job.dump.accent}>
  {#key job.id}
    <div
      class="art"
      class:fallback={!art?.hero}
      style:background-image={art?.hero ? `url(${art.hero})` : null}
    ></div>
  {/key}
  <div class="inner">
    {#key job.id}
      <div class="eyebrow reveal" style="animation-delay:.05s">
        {#if art?.icon}<img src={art.icon} alt="" />{/if}
        <span class="mono">{job.dump.titleId}{job.dump.version ? ` · v${job.dump.version}` : ""}</span>
      </div>
      <h1 class="reveal" style="animation-delay:.12s">{job.dump.titleName}</h1>
      <div class="facts reveal" style="animation-delay:.2s">
        <span><b class="num">{gb(job.dump.totalBytes)}</b> source</span>
        <span><b class="num">{n(job.dump.fileCount)}</b> files</span>
        {#if job.dump.apr}<span>APR Emu{job.dump.hasAmprIndex ? "" : " · index will be built"}</span>{/if}
        <span class="path mono" title={job.dump.path}>{job.dump.path}</span>
      </div>
    {/key}
  </div>
</section>

<style>
  .hero { position: relative; height: clamp(260px, 40vh, 440px); overflow: hidden; isolation: isolate; flex-shrink: 0; }
  .art {
    position: absolute; inset: 0; background: 62% 30% / cover no-repeat; z-index: -2;
    -webkit-mask-image: linear-gradient(90deg, transparent 0%, #000 42%), linear-gradient(0deg, transparent 0%, #000 45%);
    -webkit-mask-composite: source-in; mask-composite: intersect;
    mask-image: linear-gradient(90deg, transparent 0%, #000 42%), linear-gradient(0deg, transparent 0%, #000 45%);
    animation: settle 2.2s var(--ease) both;
  }
  .art.fallback {
    background-image:
      radial-gradient(60% 80% at 75% 20%, color-mix(in srgb, var(--accent) 45%, transparent), transparent 70%),
      radial-gradient(40% 60% at 95% 70%, color-mix(in srgb, var(--accent) 25%, transparent), transparent 70%);
  }
  @keyframes settle { from { transform: scale(1.06); opacity: 0; } 30% { opacity: 1; } to { transform: scale(1); opacity: 1; } }
  .hero::after {
    content: ""; position: absolute; inset: 40% 0 0 0; z-index: -1; pointer-events: none;
    background: linear-gradient(0deg, var(--bg) 0%, rgba(11, 11, 12, 0.72) 45%, transparent 100%);
  }
  .inner { position: absolute; left: 40px; right: 40px; bottom: 30px; }
  .eyebrow { display: flex; gap: 10px; align-items: center; color: var(--muted); margin-bottom: 14px; }
  .eyebrow img { width: 30px; height: 30px; border-radius: 7px; }
  h1 { font-weight: 600; font-size: clamp(32px, 5vw, 58px); line-height: 0.98; letter-spacing: -0.035em; max-width: 18ch; }
  .facts { display: flex; flex-wrap: wrap; margin-top: 16px; color: var(--muted); font-size: 13px; row-gap: 6px; }
  .facts span + span { padding-left: 14px; margin-left: 14px; border-left: 1px solid var(--line-strong); }
  .facts b { color: var(--text); font-weight: 500; }
  .path { color: var(--faint); max-width: 44ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 11.5px; align-self: center; }
  @media (max-width: 900px) {
    .inner { left: 20px; right: 20px; bottom: 22px; }
    .path { display: none; }
  }
</style>
