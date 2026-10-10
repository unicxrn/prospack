<script>
  let { engine, installing, installLog, dragging, onAdd, onInstall } = $props();
  const steps = [
    ["Compress", "MkPFS packs the dump into one .ffpfsc image on this PC."],
    ["Verify", "Every file is read back out of the image and compared byte for byte."],
    ["Upload", "One file over FTP, resumable, never overwriting what's on the PS5."],
    ["Mount", "ShadowMountPlus gets the same hints Game Compressor writes."],
  ];
</script>

<div class="empty">
  {#if engine && !engine.ready}
    <section class="setup reveal">
      <div class="kicker">One-time setup</div>
      <h2>Install the compression engine</h2>
      <p>
        Packer uses <b>MkPFS 1.1.0</b>, the same format Game Compressor builds on. It goes into a private Python
        environment in <span class="mono">{engine.dir}</span> and doesn't touch your system Python.
      </p>
      {#if !engine.python}
        <p class="err">Python 3 wasn't found. Install it with your package manager, then restart Packer.</p>
      {:else}
        <button class="btn primary" disabled={installing} onclick={onInstall}>
          {installing ? "Installing…" : "Install MkPFS"}
        </button>
      {/if}
      {#if installLog.length}
        <pre class="mono">{installLog.slice(-6).join("\n")}</pre>
      {/if}
    </section>
  {/if}

  <button class="drop reveal" class:dragging class:disabled={!engine?.ready} onclick={onAdd} style="animation-delay:.08s">
    <svg width="40" height="40" viewBox="0 0 40 40" fill="none" aria-hidden="true">
      <path d="M6 13.5a3 3 0 0 1 3-3h7l3 3h12a3 3 0 0 1 3 3V29a3 3 0 0 1-3 3H9a3 3 0 0 1-3-3V13.5Z" stroke="currentColor" stroke-width="1.5" />
      <path d="M20 18v9M15.5 22.5 20 18l4.5 4.5" stroke="var(--accent)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
    <h1>{dragging ? "Let go to add it" : "Drop a game dump here"}</h1>
    <p>Or click to choose a folder, such as <span class="mono">PPSA01234-app</span>. Packer only reads it.</p>
  </button>

  <ol class="steps reveal" style="animation-delay:.16s">
    {#each steps as [t, d], i}
      <li><span class="mono idx">0{i + 1}</span><b>{t}</b><span>{d}</span></li>
    {/each}
  </ol>
</div>

<style>
  .empty { max-width: 920px; margin: 0 auto; padding: 56px 40px; display: flex; flex-direction: column; gap: 28px; }
  .setup { border: 1px solid var(--line); border-radius: 14px; padding: 26px 28px; background: var(--surface); }
  .setup h2 { font-weight: 500; font-size: 22px; letter-spacing: -0.02em; margin: 10px 0 8px; }
  .setup p { color: var(--muted); max-width: 64ch; margin-bottom: 16px; }
  .setup b { color: var(--text); font-weight: 500; }
  .err { color: var(--err) !important; }
  pre { margin-top: 16px; color: var(--muted); white-space: pre-wrap; max-height: 120px; overflow: hidden; }

  .drop {
    position: relative; border: 1px dashed var(--line-strong); border-radius: 18px; background: none;
    padding: 64px 32px; display: flex; flex-direction: column; align-items: center; gap: 12px; text-align: center;
    color: var(--muted); cursor: pointer; transition: border-color 0.3s, background 0.3s, transform 0.3s var(--ease);
  }
  .drop:hover, .drop.dragging { border-color: color-mix(in srgb, var(--accent) 70%, transparent); background: color-mix(in srgb, var(--accent) 5%, transparent); }
  .drop.dragging { transform: scale(1.01); }
  .drop.disabled { pointer-events: none; opacity: 0.4; }
  .drop svg { color: var(--text); margin-bottom: 6px; transition: transform 0.4s var(--ease); }
  .drop:hover svg, .drop.dragging svg { transform: translateY(-3px); }
  .drop h1 { color: var(--text); font-weight: 500; font-size: 26px; letter-spacing: -0.03em; }

  .steps { list-style: none; display: grid; grid-template-columns: repeat(4, 1fr); border: 1px solid var(--line); border-radius: 14px; overflow: hidden; }
  .steps li { padding: 18px; display: flex; flex-direction: column; gap: 6px; color: var(--muted); font-size: 12.5px; }
  .steps li + li { border-left: 1px solid var(--line); }
  .steps b { color: var(--text); font-weight: 500; font-size: 14px; }
  .idx { color: var(--faint); font-size: 11px; margin-bottom: 6px; }
  @media (max-width: 900px) {
    .empty { padding: 28px 16px; }
    .steps { grid-template-columns: 1fr 1fr; }
    .steps li:nth-child(3) { border-left: 0; }
    .steps li:nth-child(n + 3) { border-top: 1px solid var(--line); }
  }
</style>
