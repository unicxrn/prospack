<script>
  import { flip } from "svelte/animate";
  import { fly } from "svelte/transition";
  import { gb } from "./format.js";

  let { jobs, selected, art, ps5, onSelect, onAdd } = $props();

  const verbs = { compress: "Compressing", verify: "Verifying", upload: "Uploading", mount: "Mounting" };

  function sub(j) {
    switch (j.status) {
      case "running": return { text: verbs[j.stages[j.stage].key], pct: j.stage === 3 ? null : Math.floor(j.pct) + "%" };
      case "queued": return { text: "Queued" };
      case "ready": return { text: "Verified · ready to upload", tone: "ok" };
      case "done": return { text: "On PS5", tone: "ok", pct: gb(j.imageBytes) };
      case "failed": return { text: "Failed", tone: "err" };
      default: return { text: "Cancelled", tone: "muted" };
    }
  }

  const overall = (j) => (j.status === "done" ? 100 : (j.stage * 100 + (j.status === "running" ? j.pct : 0)) / 4);
  const initials = (name) => name.split(/\s+/).filter((w) => /^[A-Za-z0-9]/.test(w)).slice(0, 2).map((w) => w[0].toUpperCase()).join("");
  const used = $derived(ps5?.totalBytes && ps5?.freeBytes != null ? ps5.totalBytes - ps5.freeBytes : null);
</script>

<aside class="rail">
  <h6><span>Queue</span><span class="num">{jobs.length}</span></h6>
  <div class="list">
    {#each jobs as j (j.id)}
      {@const s = sub(j)}
      <button
        class="q"
        class:active={j.id === selected}
        style:--accent={j.dump.accent}
        onclick={() => onSelect(j.id)}
        animate:flip={{ duration: 300 }}
        in:fly={{ y: 8, duration: 400 }}
      >
        {#if art[j.id]?.icon}
          <img src={art[j.id].icon} alt="" />
        {:else}
          <div class="ph">{initials(j.dump.titleName)}</div>
        {/if}
        <div class="txt">
          <div class="name">{j.dump.titleName}</div>
          <div class="sub {s.tone ?? ''}">
            <span>{s.text}</span>
            {#if s.pct}<span class="num">{s.pct}</span>{/if}
          </div>
          {#if j.status === "running" || j.status === "queued" || j.status === "ready"}
            <div class="mini"><i style:width="{overall(j)}%"></i></div>
          {/if}
        </div>
      </button>
    {/each}
  </div>

  <button class="add" onclick={onAdd}>
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M7 2v10M2 7h10" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" /></svg>
    Add a dump folder
    <kbd>Ctrl O</kbd>
  </button>

  {#if used != null}
    <div class="storage">
      <div class="row"><span>PS5 internal</span><b class="num">{gb(ps5.freeBytes)} free</b></div>
      <div class="meter"><i style:width="{(used / ps5.totalBytes) * 100}%"></i></div>
    </div>
  {/if}
</aside>

<style>
  .rail {
    border-right: 1px solid var(--line); padding: 18px 12px; display: flex; flex-direction: column; gap: 4px;
    min-height: 0; overflow: hidden;
  }
  h6 {
    font: 500 11px/1 var(--mono); text-transform: uppercase; letter-spacing: 0.08em; color: var(--faint);
    padding: 0 8px 10px; display: flex; justify-content: space-between;
  }
  .list { display: flex; flex-direction: column; gap: 4px; overflow-y: auto; min-height: 0; margin: 0 -4px; padding: 0 4px; }
  .q {
    display: grid; grid-template-columns: 40px 1fr; gap: 11px; align-items: center; text-align: left;
    padding: 8px; border-radius: 10px; cursor: pointer; position: relative; border: 0; background: none;
    transition: background 0.25s var(--ease);
  }
  .q:hover { background: var(--surface); }
  .q.active { background: var(--raised); }
  .q.active::before {
    content: ""; position: absolute; left: -8px; top: 12px; bottom: 12px; width: 2px; border-radius: 2px; background: var(--accent);
  }
  img, .ph { width: 40px; height: 40px; border-radius: 8px; object-fit: cover; display: block; }
  .ph { background: #1c1c20; display: grid; place-items: center; font-weight: 600; font-size: 12px; color: var(--muted); }
  .txt { min-width: 0; }
  .name { font-weight: 500; font-size: 13.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub { color: var(--muted); font-size: 12px; margin-top: 1px; display: flex; gap: 6px; }
  .sub.ok span:first-child { color: var(--ok); }
  .sub.err span:first-child { color: var(--err); }
  .sub.muted { color: var(--faint); }
  .mini { height: 2px; background: var(--line); border-radius: 2px; margin-top: 7px; overflow: hidden; }
  .mini i { display: block; height: 100%; background: var(--accent); transition: width 0.6s var(--ease); }
  .add {
    margin-top: 6px; padding: 12px; border: 1px dashed var(--line-strong); border-radius: 10px; background: none;
    color: var(--muted); font-size: 12.5px; display: flex; gap: 10px; align-items: center; cursor: pointer;
    transition: border-color 0.25s, color 0.25s, background 0.25s;
  }
  .add:hover { border-color: rgba(255, 255, 255, 0.22); color: var(--text); background: var(--surface); }
  kbd { margin-left: auto; font: 500 10.5px var(--mono); color: var(--faint); border: 1px solid var(--line-strong); border-radius: 4px; padding: 1px 5px; }
  .storage { margin-top: auto; padding: 14px 8px 4px; border-top: 1px solid var(--line); }
  .row { display: flex; justify-content: space-between; font-size: 12px; color: var(--muted); margin-bottom: 8px; }
  .row b { color: var(--text); font-weight: 500; }
  .meter { height: 4px; border-radius: 4px; background: var(--line); overflow: hidden; }
  .meter i { display: block; height: 100%; background: #3a3a40; }

  @media (max-width: 900px) {
    .rail { flex-direction: row; border-right: 0; border-bottom: 1px solid var(--line); padding: 10px 12px; align-items: stretch; overflow-x: auto; }
    h6, .storage, kbd { display: none; }
    .list { flex-direction: row; overflow: visible; }
    .q { min-width: 220px; }
    .q.active::before { left: 12px; right: 12px; top: auto; bottom: -10px; width: auto; height: 2px; }
    .add { margin: 0; min-width: 170px; }
  }
</style>
