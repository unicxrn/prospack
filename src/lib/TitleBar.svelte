<script>
  import { api, isTauri } from "./api.js";
  import { gb } from "./format.js";
  import { updates } from "./updates.svelte.js";

  let { ps5, host, onSettings, onHelp, onUpdate } = $props();
</script>

<header class="bar" data-tauri-drag-region>
  <div class="mark" data-tauri-drag-region>
    <svg width="18" height="18" viewBox="0 0 28 28" fill="none" aria-hidden="true">
      <rect x="5" y="5" width="18" height="18" rx="4" stroke="currentColor" stroke-width="2" />
      <path d="M10 14h8M14 10v8" stroke="#9b7bff" stroke-width="2" stroke-linecap="round" />
    </svg>
    Prospack
  </div>

  <button class="conn" class:off={host && ps5 && !ps5.online} class:unset={!host} onclick={onSettings} title={ps5?.error ?? ""}>
    <span class="dot" class:checking={!ps5}></span>
    {#if !host}
      <span>No PS5 set up</span>
    {:else if !ps5}
      <span>Looking for PS5…</span>
    {:else if ps5.online}
      <span>PS5 connected</span>
      <span class="sep"></span><span class="mono ip">{host}</span>
      {#if ps5.freeBytes != null}<span class="sep"></span><span class="num">{gb(ps5.freeBytes)} free</span>{/if}
    {:else}
      <span>PS5 offline</span><span class="sep"></span><span class="mono ip">{host}</span>
    {/if}
  </button>

  {#if updates.available}
    <button class="update" onclick={onUpdate} title="Prospack {updates.available.version} is available">
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true"><path d="M6 1.5v6.5M3.2 5.4 6 8.2l2.8-2.8M2 10.5h8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>
      Update to {updates.available.version}
    </button>
  {/if}
  <button class="iconbtn" aria-label="Help" title="Help (F1)" onclick={onHelp}>
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="6.3" stroke="currentColor" stroke-width="1.3"/><path d="M6.2 6.3a1.9 1.9 0 0 1 3.7.4c0 1.2-1.9 1.5-1.9 2.6" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/><circle cx="8" cy="11.4" r=".75" fill="currentColor"/></svg>
  </button>
  <button class="iconbtn" aria-label="Settings" title="Settings (Ctrl ,)" onclick={onSettings}>
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="2.2" stroke="currentColor" stroke-width="1.3"/><path d="M8 1.5v1.8M8 12.7v1.8M14.5 8h-1.8M3.3 8H1.5M12.6 3.4l-1.3 1.3M4.7 11.3l-1.3 1.3M12.6 12.6l-1.3-1.3M4.7 4.7L3.4 3.4" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>
  </button>

  {#if isTauri}
    <div class="win">
      <button class="iconbtn" aria-label="Minimize" onclick={api.window.minimize}>
        <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 6h8" stroke="currentColor" stroke-width="1.2"/></svg>
      </button>
      <button class="iconbtn" aria-label="Maximize" onclick={api.window.toggleMaximize}>
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none"><rect x="2.5" y="2.5" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.2"/></svg>
      </button>
      <button class="iconbtn close" aria-label="Close" onclick={api.window.close}>
        <svg width="12" height="12" viewBox="0 0 12 12"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/></svg>
      </button>
    </div>
  {/if}
</header>

<style>
  .bar {
    height: 48px; display: flex; align-items: center; gap: 12px; padding: 0 10px 0 18px;
    border-bottom: 1px solid var(--line); background: var(--bg); position: relative; z-index: 10;
  }
  .mark { font-weight: 600; letter-spacing: -0.02em; display: flex; align-items: center; gap: 9px; }
  .conn {
    margin-left: auto; display: flex; align-items: center; gap: 10px; color: var(--muted); font-size: 12.5px;
    background: none; border: 0; padding: 6px 10px; border-radius: 8px; cursor: pointer; transition: background 0.2s;
  }
  .conn:hover { background: var(--surface); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--ok); position: relative; }
  .dot::after {
    content: ""; position: absolute; inset: -4px; border-radius: 50%; border: 1px solid currentColor;
    color: var(--ok); opacity: 0; animation: ping 2.8s var(--ease) infinite;
  }
  .dot.checking { background: var(--faint); }
  .dot.checking::after { color: var(--faint); }
  .off .dot { background: var(--err); }
  .unset .dot { background: var(--faint); }
  .unset .dot::after { display: none; }
  .off .dot::after { display: none; }
  @keyframes ping { 0% { transform: scale(0.4); opacity: 0.7; } 80%, 100% { transform: scale(1.4); opacity: 0; } }
  .update {
    display: inline-flex; align-items: center; gap: 6px; height: 26px; padding: 0 10px; border-radius: 99px; cursor: pointer;
    border: 1px solid color-mix(in srgb, #9b7bff 45%, transparent); background: color-mix(in srgb, #9b7bff 14%, transparent);
    color: #d9ccff; font-size: 12px; font-weight: 500; animation: arrive 0.5s var(--ease);
    transition: background 0.2s;
  }
  .update:hover { background: color-mix(in srgb, #9b7bff 24%, transparent); }
  @keyframes arrive { from { opacity: 0; transform: translateY(-4px); } }
  .sep { width: 1px; height: 14px; background: var(--line-strong); }
  .win { display: flex; gap: 2px; margin-left: 6px; padding-left: 10px; border-left: 1px solid var(--line); }
  .close:hover { background: var(--err); color: #fff; }
  @media (max-width: 720px) { .ip, .sep { display: none; } }
</style>
