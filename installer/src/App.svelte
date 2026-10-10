<script>
  import { onMount } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { api, isTauri } from "./api.js";

  // welcome → options → installing → done | error
  let screen = $state("welcome");
  let info = $state(null);
  let dir = $state("");
  let startMenu = $state(true);
  let desktop = $state(true);
  let openAfter = $state(true);
  let pct = $state(0);
  let label = $state("Preparing");
  let outcome = $state(null);

  const STEPS = ["Welcome", "Options", "Install", "Done"];
  const stepIndex = $derived({ welcome: 0, options: 1, installing: 2, done: 3, error: 2 }[screen]);
  const updating = $derived(!!info?.existing);

  onMount(async () => {
    info = await api.info();
    dir = info.existing?.dir || info.defaultDir;
    await api.onProgress((p) => {
      pct = p.pct;
      label = p.label;
    });
  });

  async function browse() {
    const p = await api.pickDir();
    if (p) dir = p.endsWith("Prospack") ? p : `${p}\\Prospack`;
  }

  async function install() {
    screen = "installing";
    pct = 0;
    label = "Preparing";
    outcome = await api.install(dir, startMenu, desktop);
    if (outcome.ok) {
      pct = 100;
      setTimeout(() => {
        screen = "done";
        if (openAfter && isTauri) setTimeout(() => api.launch(outcome.exe), 900);
      }, 400);
    } else {
      screen = "error";
    }
  }
</script>

<div class="win">
  <aside class="side">
    <div class="glow"></div>
    <div class="dots"></div>
    <div class="brand" data-tauri-drag-region>
      <div class="ico">
        <svg width="18" height="18" viewBox="0 0 28 28" fill="none" aria-hidden="true">
          <rect x="5" y="5" width="18" height="18" rx="4" stroke="#ecebe7" stroke-width="2.2" />
          <path d="M10 14h8M14 10v8" stroke="#9b7bff" stroke-width="2.2" stroke-linecap="round" />
        </svg>
      </div>
      <div>
        <div class="name">Prospack</div>
        <div class="ver mono">Setup · v{info?.version ?? "…"}</div>
      </div>
    </div>
    <ol class="steps">
      {#each STEPS as s, i}
        <li class:done={i < stepIndex} class:on={i === stepIndex}><i></i>{s}</li>
      {/each}
    </ol>
  </aside>

  <main data-tauri-drag-region>
    <button class="close" aria-label="Close" onclick={api.close} disabled={screen === "installing"}>
      <svg width="11" height="11" viewBox="0 0 12 12"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" /></svg>
    </button>

    {#key screen}
      <section class="pane" in:fly={{ x: 22, duration: 360, delay: 80 }} out:fade={{ duration: 100 }}>
        {#if screen === "welcome"}
          <div class="kicker">{updating ? "Update" : "Welcome"}</div>
          <h1>{updating ? `Update Prospack to v${info.version}` : "Pack, verify and send PS5 dumps from your PC"}</h1>
          <p>
            {#if updating}
              You have v{info.existing.version || "?"} installed. Your settings, images and queue history stay as they are.
            {:else}
              Prospack compresses a game dump into one .ffpfsc image, checks every file, and uploads it to your PS5 for ShadowMountPlus.
            {/if}
          </p>
          <div class="feat">
            <div><b>Often 40–60% smaller</b>Compressed on your PC</div>
            <div><b>Verified</b>File by file, before upload</div>
            <div><b>One file</b>Fast, resumable FTP</div>
            <div><b>No Python</b>Everything included</div>
          </div>
          <footer>
            <span class="fine mono">GPL-3.0 · github.com/unicxrn/prospack</span>
            <button class="btn primary" onclick={() => (updating ? install() : (screen = "options"))}>{updating ? "Update" : "Continue"}</button>
          </footer>
        {:else if screen === "options"}
          <div class="kicker">Options</div>
          <h1>Where should Prospack go?</h1>
          <p>Installs for your user only. No admin prompt needed.</p>
          <div class="field">
            <span class="mono path" title={dir}>{dir}</span>
            <button class="link" onclick={browse}>Change</button>
          </div>
          <label class="opt"><input type="checkbox" bind:checked={startMenu} /><span class="sw"></span><b>Start menu shortcut</b></label>
          <label class="opt"><input type="checkbox" bind:checked={desktop} /><span class="sw"></span><b>Desktop shortcut</b></label>
          <label class="opt">
            <input type="checkbox" bind:checked={openAfter} /><span class="sw"></span>
            <span><b>Open Prospack when setup finishes</b><small>It'll walk you through connecting your PS5.</small></span>
          </label>
          <footer>
            <button class="btn ghost" onclick={() => (screen = "welcome")}>Back</button>
            <button class="btn accent" onclick={install}>Install</button>
          </footer>
        {:else if screen === "installing"}
          <div class="kicker">{updating ? "Updating" : "Installing"}</div>
          <div class="big">{Math.floor(pct)}<small>%</small></div>
          <div class="bar"><i style:width="{pct}%"></i></div>
          <div class="now mono">{label}…</div>
          <footer><span class="fine">This only takes a few seconds.</span></footer>
        {:else if screen === "done"}
          <div class="kicker">Done</div>
          <div class="check">
            <svg width="22" height="22" viewBox="0 0 22 22"><path d="M5 11.5l4 4L17 7" stroke="#52c08b" stroke-width="2.2" fill="none" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </div>
          <h1>Prospack is {updating ? "up to date" : "installed"}</h1>
          <ul class="list">
            {#if startMenu}<li>Start menu shortcut</li>{/if}
            {#if desktop}<li>Desktop shortcut</li>{/if}
            <li>Uninstall from Windows Settings → Apps</li>
          </ul>
          <footer>
            <button class="btn ghost" onclick={api.close}>Close</button>
            <button class="btn primary" onclick={() => api.launch(outcome.exe)}>Open Prospack</button>
          </footer>
        {:else}
          <div class="kicker err">Setup didn't finish</div>
          <h1>Something went wrong</h1>
          <p class="errmsg">{outcome?.message}</p>
          <p>Running setup again finishes or repairs the install.</p>
          <footer>
            <button class="btn ghost" onclick={api.close}>Close</button>
            <button class="btn primary" onclick={install}>Try again</button>
          </footer>
        {/if}
      </section>
    {/key}
  </main>
</div>

<style>
  .win { height: 100vh; display: grid; grid-template-columns: 38% 1fr; background: var(--bg); }
  .side { position: relative; overflow: hidden; border-right: 1px solid var(--line); display: flex; flex-direction: column; justify-content: space-between; padding: 20px; }
  .glow {
    position: absolute; inset: -40%; filter: blur(22px); pointer-events: none;
    background:
      radial-gradient(40% 35% at 30% 30%, rgba(155, 123, 255, 0.45), transparent 70%),
      radial-gradient(35% 30% at 70% 75%, rgba(61, 123, 255, 0.3), transparent 70%);
    animation: drift 14s ease-in-out infinite alternate;
  }
  @keyframes drift { to { transform: translate(6%, -5%) rotate(8deg); } }
  .dots {
    position: absolute; inset: 0; pointer-events: none;
    background-image: radial-gradient(rgba(255, 255, 255, 0.07) 1px, transparent 1px); background-size: 14px 14px;
    -webkit-mask-image: linear-gradient(180deg, transparent, #000 60%); mask-image: linear-gradient(180deg, transparent, #000 60%);
  }
  .brand { position: relative; display: flex; gap: 11px; align-items: center; }
  .ico { width: 36px; height: 36px; border-radius: 10px; background: var(--raised); display: grid; place-items: center; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1); }
  .name { font-weight: 600; font-size: 15px; letter-spacing: -0.02em; }
  .ver { color: var(--muted); font-size: 10.5px; margin-top: 1px; }
  .steps { position: relative; list-style: none; display: flex; flex-direction: column; gap: 9px; font-size: 12px; }
  .steps li { display: flex; gap: 9px; align-items: center; color: var(--faint); transition: color 0.3s; }
  .steps i { width: 6px; height: 6px; border-radius: 50%; background: rgba(255, 255, 255, 0.15); transition: all 0.3s var(--ease); }
  .steps .done { color: var(--muted); }
  .steps .done i { background: var(--ok); }
  .steps .on { color: var(--text); }
  .steps .on i { background: var(--accent); box-shadow: 0 0 0 3px rgba(155, 123, 255, 0.25); }

  main { position: relative; display: grid; }
  .close {
    position: absolute; top: 10px; right: 10px; z-index: 2; width: 28px; height: 28px; border-radius: 7px; border: 0;
    background: none; color: var(--muted); display: grid; place-items: center; cursor: pointer; transition: background 0.2s, color 0.2s;
  }
  .close:hover { background: var(--err); color: #fff; }
  .close:disabled { opacity: 0.3; pointer-events: none; }
  .pane { grid-area: 1 / 1; padding: 26px 28px 22px; display: flex; flex-direction: column; }
  h1 { font-weight: 500; font-size: 23px; letter-spacing: -0.03em; line-height: 1.12; margin: 10px 0 8px; max-width: 20ch; }
  p { color: var(--muted); max-width: 46ch; }
  p + p { margin-top: 8px; }
  .feat { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; margin-top: 16px; }
  .feat div { border: 1px solid var(--line); border-radius: 9px; padding: 9px 11px; color: var(--muted); font-size: 11.5px; display: flex; flex-direction: column; gap: 1px; }
  .feat b { color: var(--text); font-weight: 500; font-size: 12.5px; }
  footer { margin-top: auto; display: flex; justify-content: flex-end; align-items: center; gap: 8px; padding-top: 14px; }
  .fine { margin-right: auto; color: var(--faint); font-size: 11px; }

  .field { display: flex; justify-content: space-between; gap: 10px; align-items: center; margin: 12px 0 6px; border: 1px solid var(--line-strong); border-radius: 8px; padding: 8px 11px; background: var(--surface); }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .link { background: none; border: 0; color: var(--accent); cursor: pointer; flex: none; }
  .opt { display: flex; gap: 11px; align-items: flex-start; margin-top: 10px; cursor: pointer; }
  .opt input { position: absolute; opacity: 0; pointer-events: none; }
  .opt b { font-weight: 500; }
  .opt span:last-child { display: flex; flex-direction: column; }
  .opt small { color: var(--faint); font-size: 11.5px; }
  .sw { flex: none; width: 28px; height: 16px; border-radius: 16px; background: #2a2a30; position: relative; margin-top: 1px; transition: background 0.25s; }
  .sw::after { content: ""; position: absolute; top: 2px; left: 2px; width: 12px; height: 12px; border-radius: 50%; background: #fff; transition: left 0.25s var(--ease); }
  .opt input:checked + .sw { background: var(--accent); }
  .opt input:checked + .sw::after { left: 14px; }
  .opt input:focus-visible + .sw { outline: 2px solid var(--accent); outline-offset: 2px; }

  .big { font-weight: 300; font-size: 64px; letter-spacing: -0.05em; line-height: 1; margin-top: 26px; font-variant-numeric: tabular-nums; }
  .big small { font-size: 0.36em; color: var(--muted); margin-left: 3px; }
  .bar { height: 5px; border-radius: 5px; background: var(--line); margin-top: 18px; overflow: hidden; position: relative; }
  .bar i { position: absolute; inset: 0 auto 0 0; background: var(--accent); border-radius: 5px; transition: width 0.25s var(--ease); }
  .bar::after { content: ""; position: absolute; inset: 0; width: 30%; background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.5), transparent); animation: sweep 2s var(--ease) infinite; }
  @keyframes sweep { from { transform: translateX(-100%); } to { transform: translateX(400%); } }
  .now { color: var(--faint); margin-top: 12px; }

  .check { width: 46px; height: 46px; border-radius: 50%; background: rgba(82, 192, 139, 0.12); display: grid; place-items: center; margin-top: 12px; }
  .check path { stroke-dasharray: 30; stroke-dashoffset: 30; animation: draw 0.6s 0.2s var(--ease) forwards; }
  @keyframes draw { to { stroke-dashoffset: 0; } }
  .list { list-style: none; display: flex; flex-direction: column; gap: 6px; color: var(--muted); font-size: 12.5px; margin-top: 4px; }
  .list li::before { content: "✓  "; color: var(--ok); }
  .kicker.err { color: var(--err); }
  .errmsg { color: var(--text); user-select: text; }
</style>
