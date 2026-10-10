<script>
  import { time } from "./format.js";
  let { job } = $props();
  let expanded = $state(false);
  const lines = $derived([...job.log].reverse().slice(0, expanded ? 400 : 9));
</script>

<section class="log">
  <header>
    <span>Activity</span>
    <button class="btn ghost small" onclick={() => (expanded = !expanded)}>{expanded ? "Show less" : "Show all"}</button>
  </header>
  <ol class:expanded>
    {#each lines as l, i (l.t + ":" + (job.log.length - i) + l.text)}
      <li class={l.tone}>
        <time class="mono">{time(l.t)}</time>
        <span class="mono">{l.text}</span>
      </li>
    {/each}
  </ol>
</section>

<style>
  .log { border: 1px solid var(--line); border-radius: 14px; overflow: hidden; }
  header { display: flex; justify-content: space-between; align-items: center; padding: 8px 10px 8px 18px; border-bottom: 1px solid var(--line); }
  header span { font-size: 13px; font-weight: 500; }
  .small { height: 28px; font-size: 12px; }
  ol { list-style: none; padding: 8px 0; max-height: 230px; overflow: hidden; -webkit-mask-image: linear-gradient(180deg, #000 70%, transparent); mask-image: linear-gradient(180deg, #000 70%, transparent); }
  ol.expanded { max-height: 420px; overflow-y: auto; -webkit-mask-image: none; mask-image: none; }
  li { display: grid; grid-template-columns: 74px 1fr; gap: 10px; padding: 4px 18px; color: var(--muted); animation: in 0.5s var(--ease); }
  li span { user-select: text; word-break: break-word; }
  time { color: var(--faint); }
  .hl span { color: var(--text); }
  .ok span { color: var(--ok); }
  .warn span { color: var(--warn); }
  .err span { color: var(--err); }
  @keyframes in { from { opacity: 0; transform: translateY(6px); } }
</style>
