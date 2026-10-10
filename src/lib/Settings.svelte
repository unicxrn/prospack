<script>
  import { fade, fly } from "svelte/transition";
  import { api } from "./api.js";
  import { gb } from "./format.js";

  let { settings, onClose, onSaved, onRerunSetup } = $props();
  let draft = $state(structuredClone($state.snapshot(settings)));
  let testing = $state(false);
  let result = $state(null);
  let saving = $state(false);

  async function test() {
    testing = true;
    result = null;
    try {
      result = await api.ps5Status($state.snapshot(draft));
    } catch (e) {
      result = { online: false, error: String(e) };
    }
    testing = false;
  }

  async function browse() {
    const p = await api.pickFolder("Where should Prospack write images?");
    if (p) draft.outputDir = p;
  }

  async function save() {
    saving = true;
    await api.saveSettings(draft);
    saving = false;
    onSaved(draft);
    onClose();
  }

  function onKey(e) {
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="scrim" transition:fade={{ duration: 200 }} onclick={onClose} role="presentation"></div>
<aside class="drawer" transition:fly={{ x: 420, duration: 380, opacity: 1 }} aria-label="Settings">
  <header>
    <h2>Settings</h2>
    <button class="iconbtn" aria-label="Close" onclick={onClose}>
      <svg width="12" height="12" viewBox="0 0 12 12"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" /></svg>
    </button>
  </header>

  <div class="body">
    <fieldset>
      <legend class="kicker">PS5</legend>
      <div class="grid">
        <label class="wide">IP address<input bind:value={draft.ps5Host} spellcheck="false" /></label>
        <label>FTP port<input type="number" bind:value={draft.ftpPort} /></label>
        <label>Web file manager port<input type="number" bind:value={draft.webPort} /></label>
        <label class="wide">Upload folder<input class="mono" bind:value={draft.remoteDir} spellcheck="false" /></label>
      </div>
      <div class="test">
        <button class="btn" onclick={test} disabled={testing}>{testing ? "Testing…" : "Test connection"}</button>
        {#if result}
          <span class="res" class:ok={result.online} class:err={!result.online} in:fade>
            {#if result.online}
              FTP OK · {result.latencyMs} ms{result.freeBytes != null ? ` · ${gb(result.freeBytes)} free` : ""}{result.shadowmount ? " · ShadowMountPlus found" : " · ShadowMountPlus not found"}
            {:else}
              {result.error}
            {/if}
          </span>
        {/if}
      </div>
    </fieldset>

    <fieldset>
      <legend class="kicker">On this PC</legend>
      <label>Image folder
        <div class="row">
          <input class="mono" bind:value={draft.outputDir} spellcheck="false" />
          <button class="btn" onclick={browse}>Browse</button>
        </div>
        <small>Images need up to the dump's size while they're written. Avoid a drive that struggles with long writes.</small>
      </label>
      <label>
        <span class="lr"><span>Compression level</span><b class="num">{draft.compressionLevel}</b></span>
        <input type="range" min="1" max="9" bind:value={draft.compressionLevel} style:--p="{((draft.compressionLevel - 1) / 8) * 100}%" />
        <small>7 is MkPFS's and Game Compressor's default. Higher is slower for a little more saving.</small>
      </label>
      <div class="grid">
        <label>Compression threads<input type="number" min="0" bind:value={draft.cpuCount} /><small>0 = automatic</small></label>
        <label>Verify workers<input type="number" min="0" bind:value={draft.verifyWorkers} /><small>0 = automatic</small></label>
      </div>
    </fieldset>

    <fieldset>
      <legend class="kicker">Behaviour</legend>
      <label class="toggle">
        <input type="checkbox" bind:checked={draft.autoUpload} /><span class="sw"></span>
        <span><b>Upload when verified</b><small>Otherwise Prospack stops after Verify and waits for you.</small></span>
      </label>
      <label class="toggle">
        <input type="checkbox" bind:checked={draft.writeSmpHints} /><span class="sw"></span>
        <span><b>Write ShadowMountPlus settings</b><small>Adds image_sector=…:65536 and image_ro=… like Game Compressor. The previous files are backed up first.</small></span>
      </label>
      <label class="toggle">
        <input type="checkbox" bind:checked={draft.markGcValidated} /><span class="sw"></span>
        <span><b>Show as validated in Game Compressor</b><small>Writes Game Compressor's validation record after Prospack's own file-by-file check, with the original size and space saved.</small></span>
      </label>
      <label class="toggle">
        <input type="checkbox" bind:checked={draft.checkUpdates} /><span class="sw"></span>
        <span><b>Check for updates</b><small>Looks for a new Prospack release when it starts and every six hours. Nothing installs without you clicking it.</small></span>
      </label>
      <label class="toggle">
        <input type="checkbox" bind:checked={draft.deleteLocalAfterUpload} /><span class="sw"></span>
        <span><b>Delete the local image after it mounts</b><small>Only once ShadowMountPlus has confirmed the mount.</small></span>
      </label>
    </fieldset>
  </div>

  <footer>
    <button class="btn ghost rerun" onclick={onRerunSetup}>Run setup again</button>
    <button class="btn ghost" onclick={onClose}>Cancel</button>
    <button class="btn primary" onclick={save} disabled={saving}>Save</button>
  </footer>
</aside>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.45); z-index: 40; backdrop-filter: blur(2px); }
  .drawer {
    position: fixed; top: 0; right: 0; bottom: 0; width: min(440px, 100vw); z-index: 41;
    background: var(--surface); border-left: 1px solid var(--line-strong); display: flex; flex-direction: column;
    box-shadow: -30px 0 60px rgba(0, 0, 0, 0.35);
  }
  header { display: flex; justify-content: space-between; align-items: center; padding: 18px 18px 14px 24px; }
  h2 { font-weight: 500; font-size: 18px; letter-spacing: -0.02em; }
  .body { flex: 1; overflow-y: auto; padding: 4px 24px 24px; display: flex; flex-direction: column; gap: 26px; }
  fieldset { border: 0; display: flex; flex-direction: column; gap: 14px; }
  legend { margin-bottom: 12px; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  .wide { grid-column: 1 / -1; }
  label { display: flex; flex-direction: column; gap: 6px; font-size: 12.5px; color: var(--muted); }
  label b { color: var(--text); font-weight: 500; }
  small { color: var(--faint); font-size: 11.5px; line-height: 1.4; }
  input:not([type="checkbox"]):not([type="range"]) {
    height: 34px; padding: 0 10px; border-radius: 8px; border: 1px solid var(--line-strong); background: var(--bg);
    font-size: 13px; outline: none; transition: border-color 0.2s; width: 100%;
  }
  input:focus { border-color: color-mix(in srgb, var(--accent) 70%, transparent); }
  input.mono { font-family: var(--mono); font-size: 12px; }
  .lr { display: flex; justify-content: space-between; }
  input[type="range"] {
    -webkit-appearance: none; appearance: none; height: 4px; border-radius: 4px; margin: 8px 0 2px;
    background: linear-gradient(90deg, var(--accent) var(--p), var(--line-strong) var(--p));
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none; width: 16px; height: 16px; border-radius: 50%; background: #fff;
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 25%, transparent); cursor: pointer; transition: transform 0.15s;
  }
  input[type="range"]::-webkit-slider-thumb:hover { transform: scale(1.12); }
  .row { display: flex; gap: 8px; }
  .test { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; }
  .res { font-size: 12px; }
  .res.ok { color: var(--ok); }
  .res.err { color: var(--err); }

  .toggle { flex-direction: row; align-items: flex-start; gap: 12px; cursor: pointer; }
  .toggle input { position: absolute; opacity: 0; pointer-events: none; }
  .toggle > span:last-child { display: flex; flex-direction: column; gap: 2px; }
  .sw { flex: none; width: 30px; height: 18px; border-radius: 18px; background: #2a2a30; position: relative; transition: background 0.25s; margin-top: 1px; }
  .sw::after { content: ""; position: absolute; top: 2px; left: 2px; width: 14px; height: 14px; border-radius: 50%; background: #fff; transition: left 0.25s var(--ease); }
  .toggle input:checked + .sw { background: var(--accent); }
  .toggle input:checked + .sw::after { left: 14px; }
  .toggle input:focus-visible + .sw { outline: 2px solid var(--accent); outline-offset: 2px; }

  .rerun { margin-right: auto; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding: 14px 24px; border-top: 1px solid var(--line); }
</style>
