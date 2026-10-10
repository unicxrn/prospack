<script>
  import { onMount } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { api } from "./api.js";
  import { gb } from "./format.js";

  let { settings, onDone } = $props();

  let step = $state(0);
  let dir = $state(1);
  let draft = $state(structuredClone($state.snapshot(settings)));

  // Step 2: find the PS5
  let scanning = $state(false);
  let scanned = $state(false);
  let found = $state([]);
  let manual = $state(false);
  let manualHost = $state(draft.ps5Host || "");
  let manualPort = $state(draft.ftpPort || 2121);
  let checking = $state(false);
  let checks = $state(null);

  // Step 3: storage
  let free = $state(null);

  const STEPS = ["Welcome", "Find your PS5", "Storage", "Ready"];

  function go(n) {
    dir = n > step ? 1 : -1;
    step = n;
    if (n === 1 && !scanned) scan();
    if (n === 2) refreshFree();
  }

  async function scan() {
    scanning = true;
    checks = null;
    try {
      found = await api.discoverPs5();
    } catch {
      found = [];
    }
    scanning = false;
    scanned = true;
    const preferred = found.find((f) => f.ip === draft.ps5Host) ?? found[0];
    if (preferred && !manual) choose(preferred.ip, preferred.port);
    else if (!found.length) manual = true;
  }

  async function choose(host, port) {
    draft.ps5Host = host.trim();
    draft.ftpPort = Number(port) || 2121;
    checking = true;
    checks = null;
    try {
      checks = await api.ps5Status($state.snapshot(draft));
    } catch (e) {
      checks = { online: false, error: String(e) };
    }
    checking = false;
  }

  async function refreshFree() {
    try {
      free = await api.freeSpace(draft.outputDir);
    } catch {
      free = null;
    }
  }

  async function browse() {
    const p = await api.pickFolder("Where should Prospack keep images?");
    if (p) {
      draft.outputDir = p;
      refreshFree();
    }
  }

  function finish(pickNow) {
    onDone({ ...$state.snapshot(draft), setupDone: true }, pickNow);
  }

  const canContinue = $derived(step !== 1 || (checks?.online && !checking));

  function onKey(e) {
    if (e.key === "Enter" && !e.target.closest("input") && canContinue && step < 3) go(step + 1);
  }

  onMount(() => {
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="wrap">
  <section class="card" in:fly={{ y: 14, duration: 500 }}>
    <header>
      <span class="kicker">{step === 0 ? "Welcome" : step === 3 ? "All set" : `Step ${step + 1} of 4`}</span>
      <div class="dots" aria-label="Setup progress">
        {#each STEPS as s, i}<i class:on={i <= step} title={s}></i>{/each}
      </div>
    </header>

    <div class="body">
      {#key step}
        <div class="pane" in:fly={{ x: 28 * dir, duration: 380, delay: 60 }} out:fade={{ duration: 120 }}>
          {#if step === 0}
            <h1>Put PS5 games on your console in one file</h1>
            <p class="lead">
              Prospack turns a game dump into a compressed <span class="mono">.ffpfsc</span> image on your PC, checks it,
              and sends it to your PS5, where ShadowMountPlus mounts it.
            </p>
            <div class="tiles">
              <div><b>Compress</b><span>On your PC, often 40–60% smaller</span></div>
              <div><b>Verify</b><span>Every file checked against the dump</span></div>
              <div><b>Upload</b><span>One file over FTP, resumable</span></div>
              <div><b>Mount</b><span>Shows up on the PS5</span></div>
            </div>
            <div class="kicker sub">You'll need</div>
            <ul class="req">
              <li>A jailbroken PS5 running an FTP server (ftpsrv or etaHEN)</li>
              <li>ShadowMountPlus running on the PS5</li>
              <li>A game dump folder, such as <span class="mono">PPSA01234-app</span></li>
            </ul>
          {:else if step === 1}
            <h1>Find your PS5</h1>
            <p class="lead">Turn the PS5 on, load your FTP server, and make sure it's on the same network as this PC.</p>

            {#if scanning}
              <div class="scan"><i></i></div>
              <p class="hint mono">Looking for FTP servers on your network…</p>
            {:else}
              <div class="found">
                {#each found as f (f.ip + ":" + f.port)}
                  {@const sel = !manual && draft.ps5Host === f.ip && draft.ftpPort === f.port}
                  <button class="row" class:sel onclick={() => { manual = false; choose(f.ip, f.port); }}>
                    <span><b>{f.ip}</b> <span class="mono dim">· port {f.port} · {f.label}</span></span>
                    {#if f.label !== "FTP server"}<span class="tag">PS5 found</span>{/if}
                  </button>
                {/each}
                <button class="row" class:sel={manual} onclick={() => (manual = true)}>
                  <span class="dim">{found.length ? "Not listed? Enter it yourself" : "Nothing found automatically. Enter your PS5's IP"}</span>
                </button>
                {#if manual}
                  <div class="manual" in:fade>
                    <input placeholder="192.168.1.50" bind:value={manualHost} spellcheck="false" />
                    <input class="port" type="number" bind:value={manualPort} />
                    <button class="btn" disabled={!manualHost.trim() || checking} onclick={() => choose(manualHost, manualPort)}>Check</button>
                  </div>
                {/if}
              </div>
              <button class="link" onclick={scan}>Scan again</button>
            {/if}

            {#if checking}
              <p class="hint mono" in:fade>Checking {draft.ps5Host}…</p>
            {:else if checks}
              <ul class="checks" in:fade>
                <li class={checks.online ? "ok" : "bad"}>
                  <span class="ck"></span>
                  {#if checks.online}FTP server reachable · {checks.latencyMs} ms{:else}Can't connect<small>{checks.error}</small>{/if}
                </li>
                {#if checks.online}
                  <li class={checks.shadowmount ? "ok" : "warn"}>
                    <span class="ck"></span>
                    {#if checks.shadowmount}ShadowMountPlus found{:else}ShadowMountPlus not found<small>Load ShadowMountPlus on the PS5. Without it, games won't appear.</small>{/if}
                  </li>
                  <li class={checks.gameCompressor ? "ok" : "opt"}>
                    <span class="ck"></span>
                    {#if checks.gameCompressor}Game Compressor found <span class="dim">· optional</span>{:else}Game Compressor not found <span class="dim">· optional, only for its "validated" badge</span>{/if}
                  </li>
                  {#if checks.freeBytes != null}
                    <li class="ok"><span class="ck"></span>{gb(checks.freeBytes)} free on the PS5</li>
                  {/if}
                {/if}
              </ul>
            {/if}
          {:else if step === 2}
            <h1>Where should images go?</h1>
            <p class="lead">
              Images are written on this PC before they're sent. Pick a drive with room for the biggest game you'll pack, up
              to the full size of its dump.
            </p>
            <div class="field">
              <span class="mono path" title={draft.outputDir}>{draft.outputDir}</span>
              <button class="link" onclick={browse}>Change</button>
            </div>
            <div class="freeline"><span>Free on that drive</span><b class="num">{free != null ? gb(free) : "—"}</b></div>
            <div class="kicker sub">After a game is on the PS5</div>
            <div class="choice">
              <label class:sel={!draft.deleteLocalAfterUpload}>
                <input type="radio" name="keep" checked={!draft.deleteLocalAfterUpload} onchange={() => (draft.deleteLocalAfterUpload = false)} />
                <span><b>Keep the image on this PC</b><small>Recommended while you're testing. Re-sending is instant.</small></span>
              </label>
              <label class:sel={draft.deleteLocalAfterUpload}>
                <input type="radio" name="keep" checked={draft.deleteLocalAfterUpload} onchange={() => (draft.deleteLocalAfterUpload = true)} />
                <span><b>Delete it once the PS5 confirms the game mounted</b><small>Saves space. The original dump is never touched either way.</small></span>
              </label>
            </div>
          {:else}
            <h1>Drop in your first game</h1>
            <ul class="tips">
              <li><b>Drag a dump folder</b> onto the window, or press <kbd>Ctrl O</kbd>.</li>
              <li><b>Leave it running.</b> Each game compresses, verifies, uploads and mounts on its own. Queue as many as you like.</li>
              <li><b>Nothing is deleted.</b> Your dump is only read, and Prospack never overwrites files on the PS5.</li>
              <li><b>Stuck?</b> The <kbd>?</kbd> button explains every screen and the common problems.</li>
            </ul>
          {/if}
        </div>
      {/key}
    </div>

    <footer>
      {#if step === 0}
        <button class="btn ghost" onclick={() => finish(false)}>Skip setup</button>
        <button class="btn primary" onclick={() => go(1)}>Get started</button>
      {:else if step < 3}
        <button class="btn ghost" onclick={() => go(step - 1)}>Back</button>
        <button class="btn primary" disabled={!canContinue} onclick={() => go(step + 1)}>Continue</button>
      {:else}
        <button class="btn ghost" onclick={() => finish(false)}>Not now</button>
        <button class="btn primary" onclick={() => finish(true)}>Choose a dump</button>
      {/if}
    </footer>
  </section>
</div>

<style>
  .wrap { min-height: calc(100vh - 48px); display: grid; place-items: center; padding: 40px 20px; }
  .card {
    --accent: #9b7bff;
    width: min(720px, 100%); border: 1px solid var(--line); border-radius: 16px; background: var(--surface);
    display: flex; flex-direction: column; overflow: hidden; position: relative;
    box-shadow: 0 40px 80px rgba(0, 0, 0, 0.35);
  }
  .card::before {
    content: ""; position: absolute; inset: -40% -20% auto auto; width: 70%; height: 90%; pointer-events: none;
    background: radial-gradient(closest-side, rgba(155, 123, 255, 0.16), transparent); filter: blur(10px);
  }
  header { display: flex; align-items: center; padding: 16px 22px; border-bottom: 1px solid var(--line); position: relative; }
  .dots { margin-left: auto; display: flex; gap: 5px; }
  .dots i { width: 22px; height: 3px; border-radius: 3px; background: var(--line-strong); transition: background 0.4s var(--ease); }
  .dots i.on { background: var(--accent); }
  .body { position: relative; min-height: 400px; display: grid; }
  .pane { grid-area: 1 / 1; padding: 26px 28px 22px; display: flex; flex-direction: column; gap: 12px; }
  h1 { font-weight: 500; font-size: 26px; letter-spacing: -0.03em; line-height: 1.12; }
  .lead { color: var(--muted); max-width: 58ch; }
  .kicker.sub { margin-top: 10px; }
  .dim { color: var(--faint); }
  .hint { color: var(--faint); }

  .tiles { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; margin-top: 4px; }
  .tiles div { border: 1px solid var(--line); border-radius: 10px; padding: 12px; display: flex; flex-direction: column; gap: 3px; font-size: 12px; color: var(--muted); }
  .tiles b { color: var(--text); font-weight: 500; font-size: 13px; }
  .req, .tips { list-style: none; display: flex; flex-direction: column; gap: 8px; }
  .req li { display: flex; gap: 10px; align-items: center; color: var(--muted); }
  .req li::before { content: ""; width: 12px; height: 12px; border-radius: 50%; border: 1.5px solid var(--faint); flex: none; }
  .tips li { border: 1px solid var(--line); border-radius: 10px; padding: 12px 14px; color: var(--muted); }
  .tips b { color: var(--text); font-weight: 500; }
  kbd { font: 500 11px var(--mono); border: 1px solid var(--line-strong); border-radius: 4px; padding: 1px 5px; color: var(--text); }

  .scan { height: 2px; background: var(--line); border-radius: 2px; overflow: hidden; position: relative; margin-top: 8px; }
  .scan i { position: absolute; inset: 0; width: 35%; background: var(--accent); border-radius: 2px; animation: slide 1.2s var(--ease) infinite; }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(300%); } }
  .found { border: 1px solid var(--line); border-radius: 10px; overflow: hidden; }
  .row {
    width: 100%; display: flex; justify-content: space-between; align-items: center; gap: 10px; padding: 11px 14px;
    background: none; border: 0; text-align: left; cursor: pointer; transition: background 0.2s;
  }
  .row + .row, .manual { border-top: 1px solid var(--line); }
  .row:hover { background: var(--raised); }
  .row.sel { background: var(--raised); box-shadow: inset 2px 0 0 var(--accent); }
  .row b { font-weight: 500; }
  .tag { color: var(--ok); font-size: 12px; }
  .manual { display: flex; gap: 8px; padding: 10px 14px; }
  input:not([type="radio"]) {
    height: 34px; padding: 0 10px; border-radius: 8px; border: 1px solid var(--line-strong); background: var(--bg);
    font: 12.5px var(--mono); outline: none; flex: 1; min-width: 0;
  }
  input:focus { border-color: color-mix(in srgb, var(--accent) 70%, transparent); }
  input.port { flex: 0 0 90px; }
  .link { background: none; border: 0; color: var(--accent); cursor: pointer; font-size: 12.5px; align-self: flex-start; padding: 2px 0; }
  .link:hover { text-decoration: underline; }

  .checks { list-style: none; display: flex; flex-direction: column; gap: 8px; margin-top: 4px; }
  .checks li { display: flex; gap: 10px; align-items: baseline; color: var(--muted); flex-wrap: wrap; }
  .checks small { flex-basis: 100%; padding-left: 24px; color: var(--faint); margin-top: -2px; }
  .ck { width: 14px; height: 14px; border-radius: 50%; flex: none; position: relative; top: 2px; }
  .ok .ck { background: var(--ok); }
  .ok .ck::after { content: ""; position: absolute; left: 4.5px; top: 2px; width: 3px; height: 6px; border: solid #0b0b0c; border-width: 0 1.6px 1.6px 0; transform: rotate(45deg); }
  .bad .ck { background: var(--err); }
  .warn .ck { background: var(--warn); }
  .opt .ck { border: 1.5px solid var(--faint); }
  .bad { color: var(--text) !important; }

  .field { display: flex; justify-content: space-between; align-items: center; gap: 12px; border: 1px solid var(--line-strong); border-radius: 9px; padding: 9px 12px; background: var(--bg); }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .freeline { display: flex; justify-content: space-between; color: var(--muted); font-size: 12.5px; }
  .freeline b { color: var(--text); font-weight: 500; }
  .choice { display: flex; flex-direction: column; gap: 8px; }
  .choice label { display: flex; gap: 12px; align-items: flex-start; border: 1px solid var(--line); border-radius: 10px; padding: 11px 14px; cursor: pointer; transition: border-color 0.2s, background 0.2s; }
  .choice label.sel { border-color: color-mix(in srgb, var(--accent) 55%, transparent); background: color-mix(in srgb, var(--accent) 5%, transparent); }
  .choice span { display: flex; flex-direction: column; gap: 2px; }
  .choice b { font-weight: 500; }
  .choice small { color: var(--faint); font-size: 12px; }
  .choice input { accent-color: var(--accent); margin-top: 3px; }

  footer { display: flex; justify-content: space-between; padding: 14px 22px; border-top: 1px solid var(--line); position: relative; }

  @media (max-width: 680px) {
    .tiles { grid-template-columns: 1fr 1fr; }
    .pane { padding: 22px 18px; }
    h1 { font-size: 22px; }
  }
</style>
