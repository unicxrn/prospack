<script>
  import { api, isTauri } from "./api.js";
  import { gb } from "./format.js";

  let { ps5, host, onSettings } = $props();
</script>

<header class="bar" data-tauri-drag-region>
  <div class="mark" data-tauri-drag-region>
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <rect x="1.5" y="1.5" width="15" height="15" rx="4" stroke="currentColor" stroke-width="1.4" />
      <path d="M5.5 6.5h7" stroke="var(--accent)" stroke-width="1.4" stroke-linecap="round" />
      <path d="M5.5 9.5h7M5.5 12.5h4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
    </svg>
    Packer
  </div>

  <button class="conn" class:off={ps5 && !ps5.online} onclick={onSettings} title={ps5?.error ?? ""}>
    <span class="dot" class:checking={!ps5}></span>
    {#if !ps5}
      <span>Looking for PS5…</span>
    {:else if ps5.online}
      <span>PS5 connected</span>
      <span class="sep"></span><span class="mono ip">{host}</span>
      {#if ps5.freeBytes != null}<span class="sep"></span><span class="num">{gb(ps5.freeBytes)} free</span>{/if}
    {:else}
      <span>PS5 offline</span><span class="sep"></span><span class="mono ip">{host}</span>
    {/if}
  </button>

  <button class="iconbtn" aria-label="Settings" onclick={onSettings}>
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
  .off .dot::after { display: none; }
  @keyframes ping { 0% { transform: scale(0.4); opacity: 0.7; } 80%, 100% { transform: scale(1.4); opacity: 0; } }
  .sep { width: 1px; height: 14px; background: var(--line-strong); }
  .win { display: flex; gap: 2px; margin-left: 6px; padding-left: 10px; border-left: 1px solid var(--line); }
  .close:hover { background: var(--err); color: #fff; }
  @media (max-width: 720px) { .ip, .sep { display: none; } }
</style>
