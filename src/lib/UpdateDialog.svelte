<script>
  import { fade, scale } from "svelte/transition";
  import { api } from "./api.js";
  import { updates, installUpdate, releasePage } from "./updates.svelte.js";

  let { busyGame, pending, onPending, onClose } = $props();

  const a = $derived(updates.available);
  const pct = $derived(updates.progress?.total ? (updates.progress.got / updates.progress.total) * 100 : 0);
  const mb = (b) => (b / 1e6).toFixed(1);

  function onKey(e) {
    if (e.key === "Escape" && !updates.installing) onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="scrim" transition:fade={{ duration: 180 }} onclick={() => !updates.installing && onClose()} role="presentation"></div>
<div class="dialog" role="dialog" aria-label="Update Prospack" transition:scale={{ start: 0.96, duration: 260, opacity: 0 }}>
  {#if a}
    <div class="kicker">Update available</div>
    <h2>Prospack {a.version}</h2>
    <p class="sub">You have {updates.current}.</p>

    {#if a.notes.trim()}
      <div class="notes">
        {#each a.notes.split("\n").map((l) => l.replace(/^[-*]\s*/, "").trim()).filter(Boolean) as line}
          <p>{line}</p>
        {/each}
      </div>
    {/if}

    {#if updates.installing}
      <div class="bar"><i style:width="{pct}%"></i></div>
      <p class="sub mono">
        {#if updates.progress?.total && updates.progress.got < updates.progress.total}
          Downloading {mb(updates.progress.got)} of {mb(updates.progress.total)} MB
        {:else}
          Installing. Prospack will restart in a moment.
        {/if}
      </p>
    {:else if updates.lastError}
      <p class="err">The update didn't install: {updates.lastError}</p>
    {/if}

    <footer>
      {#if !a.canInstall}
        <span class="hint">This copy was installed from a .deb, so it can't update itself.</span>
        <button class="btn ghost" onclick={onClose}>Later</button>
        <button class="btn primary" onclick={() => api.openUrl(releasePage(a.version))}>Download {a.version}</button>
      {:else if updates.installing}
        <span class="hint">Keep Prospack open until it restarts.</span>
      {:else if busyGame}
        <span class="hint">{busyGame} is still running. The update waits until it's done.</span>
        <button class="btn ghost" onclick={onClose}>Later</button>
        <button class="btn primary" disabled={pending} onclick={onPending}>{pending ? "Waiting for the game…" : "Update when it's done"}</button>
      {:else}
        <button class="btn ghost" onclick={() => api.openUrl(releasePage(a.version))}>Release notes</button>
        <button class="btn ghost" onclick={onClose}>Later</button>
        <button class="btn primary" onclick={installUpdate}>Update and restart</button>
      {/if}
    </footer>
  {/if}
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.5); z-index: 60; backdrop-filter: blur(2px); }
  .dialog {
    position: fixed; z-index: 61; left: 50%; top: 50%; translate: -50% -50%; width: min(460px, calc(100vw - 32px));
    background: var(--surface); border: 1px solid var(--line-strong); border-radius: 16px; padding: 22px 24px 18px;
    box-shadow: 0 40px 80px rgba(0, 0, 0, 0.45); user-select: text;
  }
  .kicker { color: var(--accent); }
  h2 { font-weight: 500; font-size: 24px; letter-spacing: -0.03em; margin: 8px 0 2px; }
  .sub { color: var(--muted); font-size: 12.5px; }
  .notes { margin-top: 14px; border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; max-height: 180px; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; color: var(--muted); font-size: 12.5px; }
  .notes p { padding-left: 14px; position: relative; }
  .notes p::before { content: ""; position: absolute; left: 2px; top: 8px; width: 4px; height: 4px; border-radius: 50%; background: var(--faint); }
  .bar { height: 5px; border-radius: 5px; background: var(--line); margin: 18px 0 8px; overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--accent); transition: width 0.2s var(--ease); }
  .err { color: var(--err); font-size: 12.5px; margin-top: 12px; }
  footer { display: flex; justify-content: flex-end; align-items: center; gap: 8px; margin-top: 20px; flex-wrap: wrap; }
  .hint { margin-right: auto; color: var(--faint); font-size: 12px; max-width: 26ch; }
</style>
