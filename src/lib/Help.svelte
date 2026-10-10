<script>
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { api, isTauri } from "./api.js";
  import { updates, checkForUpdates } from "./updates.svelte.js";

  let { onClose, onRerunSetup, onShowUpdate } = $props();
  let checkedHere = $state(false);

  async function check() {
    await checkForUpdates();
    checkedHere = true;
    if (updates.available) onShowUpdate();
  }
  let version = $state("dev");
  const REPO = "https://github.com/unicxrn/prospack";

  onMount(async () => {
    if (isTauri) {
      try {
        version = await (await import("@tauri-apps/api/app")).getVersion();
      } catch {}
    }
  });

  const open = (url) => api.openUrl(url);

  const trouble = [
    {
      q: "Prospack can't find my PS5",
      a: [
        "Make sure the PS5 is on, jailbroken, and running an FTP server: ftpsrv (port 2121) or etaHEN (port 1337).",
        "The PC and the PS5 must be on the same network. Guest Wi-Fi and VPNs often keep them apart.",
        "Enter the IP yourself in Settings → Run setup again. It's under Settings → Network on the PS5.",
      ],
    },
    {
      q: "Verify failed",
      a: [
        "The image didn't match the dump, so nothing was uploaded. Choose Compress again to rebuild it.",
        "If it keeps failing, the dump itself may be changing or damaged. Copy it again, or check the drive it's on.",
      ],
    },
    {
      q: "Mount not confirmed",
      a: [
        "The image is on the PS5, but ShadowMountPlus hadn't mounted it within 5 minutes. It often shows up a bit later.",
        "Check the PS5's home screen, then use Check again. Restarting ShadowMountPlus also triggers a scan.",
      ],
    },
    {
      q: "\"The PS5 already has a different image\"",
      a: [
        "Prospack never overwrites. Delete or rename the existing .ffpfsc on the PS5 (with a file manager or FTP), then Retry.",
      ],
    },
    {
      q: "Not enough space",
      a: [
        "While it's being written, an image can need up to the dump's full size. Pick another image folder in Settings, or free up space.",
      ],
    },
    {
      q: "The game appears but won't start",
      a: [
        "That's on the PS5 side, not the image: check your ShadowMountPlus version, backports, and whether the game needs a newer firmware.",
        "If the dump runs from a plain folder but not as an image, report it with the ShadowMountPlus log (/data/shadowmount/debug.log).",
      ],
    },
  ];

  function onKey(e) {
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="scrim" transition:fade={{ duration: 200 }} onclick={onClose} role="presentation"></div>
<aside class="drawer" transition:fly={{ x: 460, duration: 380, opacity: 1 }} aria-label="Help">
  <header>
    <h2>Help</h2>
    <button class="iconbtn" aria-label="Close" onclick={onClose}>
      <svg width="12" height="12" viewBox="0 0 12 12"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" /></svg>
    </button>
  </header>

  <div class="body">
    <section>
      <div class="kicker">How it works</div>
      <ol class="steps">
        <li><b>Compress</b> MkPFS packs the dump into one <code>&lt;TITLE&gt;.ffpfsc</code> on this PC: an exFAT image inside a compressed PFS container, the same format Game Compressor makes.</li>
        <li><b>Verify</b> Every file is read back out of the image and compared with the dump, byte for byte. A single difference stops the job before anything is uploaded.</li>
        <li><b>Upload</b> One file over FTP to <code>/data/homebrew</code>, under a hidden name until it's complete, so ShadowMountPlus never sees a half-written image. Interrupted uploads resume.</li>
        <li><b>Mount</b> Prospack adds the image to ShadowMountPlus (the same settings Game Compressor writes) and waits until it's mounted.</li>
      </ol>
    </section>

    <section>
      <div class="kicker">What it changes</div>
      <ul class="plain">
        <li>Prospack only reads your dump folder. The exception is an APR Emu game without an <code>ampr_emu.index</code>: MkPFS adds one, because the game needs it.</li>
        <li>On this PC it writes the image to your image folder.</li>
        <li>On the PS5 it writes the image, and updates <code>autotune.ini</code>, <code>config.ini</code> and <code>manual.lst</code> in <code>/data/shadowmount</code>. If Game Compressor is installed, it also writes Game Compressor's record so the game shows as validated there. Each PS5 file is backed up on this PC before it's changed.</li>
      </ul>
    </section>

    <section>
      <div class="kicker">Using Prospack</div>
      <ul class="plain">
        <li>Add games by dropping dump folders on the window, with <kbd>Ctrl O</kbd>, or by passing folders on the command line. Prospack works through them one at a time.</li>
        <li>Cancel stops the current step. Resume picks up where it stopped, including a half-finished upload.</li>
        <li>Check again looks for the mount once more when ShadowMountPlus was slow.</li>
        <li>Show image opens the image folder. <kbd>Ctrl ,</kbd> opens Settings and <kbd>F1</kbd> opens this panel.</li>
      </ul>
      <button class="btn" onclick={onRerunSetup}>Run setup again</button>
    </section>

    <section>
      <div class="kicker">Troubleshooting</div>
      {#each trouble as t}
        <details>
          <summary>{t.q}</summary>
          {#each t.a as line}<p>{line}</p>{/each}
        </details>
      {/each}
    </section>

    <section class="about">
      <div class="kicker">About</div>
      <p><b>Prospack</b> {version} · GPL-3.0 · <button class="link" onclick={() => open(REPO)}>github.com/unicxrn/prospack</button></p>
      <p class="credits">
        Built on
        <button class="link" onclick={() => open("https://github.com/PSBrew/MkPFS")}>MkPFS</button>, and made to work with
        <button class="link" onclick={() => open("https://github.com/drakmor/ShadowMountPlus")}>ShadowMountPlus</button>,
        <button class="link" onclick={() => open("https://github.com/juma-sayeh/PS5-Game-Compressor")}>Game Compressor</button> and
        <button class="link" onclick={() => open("https://github.com/ps5-payload-dev/ftpsrv")}>ftpsrv</button>. Thanks to their authors.
      </p>
      <div class="upd">
        <button class="btn" onclick={check} disabled={updates.checking}>{updates.checking ? "Checking…" : "Check for updates"}</button>
        {#if checkedHere && !updates.checking}
          <span class:err={updates.lastError}>
            {#if updates.lastError}Couldn't check: {updates.lastError}{:else if updates.available}Prospack {updates.available.version} is available.{:else}You're on the latest version.{/if}
          </span>
        {/if}
      </div>
      <p class="credits">Only use dumps of games you own.</p>
    </section>
  </div>
</aside>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.45); z-index: 40; backdrop-filter: blur(2px); }
  .drawer {
    position: fixed; top: 0; right: 0; bottom: 0; width: min(480px, 100vw); z-index: 41;
    background: var(--surface); border-left: 1px solid var(--line-strong); display: flex; flex-direction: column;
    box-shadow: -30px 0 60px rgba(0, 0, 0, 0.35); user-select: text;
  }
  header { display: flex; justify-content: space-between; align-items: center; padding: 18px 18px 14px 24px; }
  h2 { font-weight: 500; font-size: 18px; letter-spacing: -0.02em; }
  .body { flex: 1; overflow-y: auto; padding: 4px 24px 28px; display: flex; flex-direction: column; gap: 26px; }
  section { display: flex; flex-direction: column; gap: 10px; color: var(--muted); font-size: 13px; line-height: 1.5; }
  b { color: var(--text); font-weight: 500; }
  code { font: 11.5px var(--mono); color: var(--text); background: var(--raised); border-radius: 4px; padding: 0 4px; }
  kbd { font: 500 10.5px var(--mono); border: 1px solid var(--line-strong); border-radius: 4px; padding: 0 4px; color: var(--text); }
  .steps { list-style: none; counter-reset: s; display: flex; flex-direction: column; gap: 10px; }
  .steps li { counter-increment: s; position: relative; padding-left: 30px; }
  .steps li::before { content: "0" counter(s); position: absolute; left: 0; top: 1px; font: 500 11px var(--mono); color: var(--faint); }
  .steps b { display: block; }
  .plain { list-style: none; display: flex; flex-direction: column; gap: 8px; }
  details { border: 1px solid var(--line); border-radius: 10px; padding: 0 14px; transition: background 0.2s; }
  details[open] { background: var(--raised); padding-bottom: 10px; }
  summary { cursor: pointer; padding: 11px 0; color: var(--text); list-style: none; display: flex; justify-content: space-between; }
  summary::-webkit-details-marker { display: none; }
  summary::after { content: "+"; color: var(--faint); font-family: var(--mono); transition: transform 0.25s var(--ease); }
  details[open] summary::after { transform: rotate(45deg); }
  details p + p { margin-top: 6px; }
  .btn { align-self: flex-start; margin-top: 4px; }
  .link { background: none; border: 0; color: var(--text); text-decoration: underline; text-decoration-color: var(--line-strong); cursor: pointer; padding: 0; font: inherit; }
  .link:hover { text-decoration-color: currentColor; }
  .credits { font-size: 12.5px; }
  .upd { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; font-size: 12.5px; }
  .upd .err { color: var(--err); }
</style>
