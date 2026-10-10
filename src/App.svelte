<script>
  import { onMount } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { api } from "./lib/api.js";
  import TitleBar from "./lib/TitleBar.svelte";
  import Queue from "./lib/Queue.svelte";
  import Hero from "./lib/Hero.svelte";
  import Pipeline from "./lib/Pipeline.svelte";
  import Focus from "./lib/Focus.svelte";
  import Activity from "./lib/Activity.svelte";
  import EmptyState from "./lib/EmptyState.svelte";
  import Settings from "./lib/Settings.svelte";
  import Setup from "./lib/Setup.svelte";
  import Help from "./lib/Help.svelte";

  let jobs = $state([]);
  let selected = $state(null);
  let art = $state({});
  let settings = $state(null);
  let engine = $state(null);
  let ps5 = $state(null);
  let showSettings = $state(false);
  let showHelp = $state(false);
  let dragging = $state(false);
  let installing = $state(false);
  let installLog = $state([]);
  let toast = $state(null);

  const job = $derived(jobs.find((j) => j.id === selected) ?? null);
  const ahead = $derived(job ? jobs.filter((j) => j.id < job.id && (j.status === "queued" || j.status === "running")).length : 0);

  function notify(text, tone = "err") {
    toast = { text, tone, id: Date.now() };
    const id = toast.id;
    setTimeout(() => toast?.id === id && (toast = null), 6000);
  }

  function setJobs(list) {
    jobs = list;
    if (selected == null || !list.some((j) => j.id === selected)) {
      selected = (list.find((j) => j.status === "running") ?? list[0])?.id ?? null;
    }
    for (const j of list) {
      if (!(j.id in art)) {
        art[j.id] = null;
        api.jobArt(j.dump.path, j.dump.titleId).then((a) => (art[j.id] = a)).catch(() => {});
      }
    }
  }

  async function add(path) {
    try {
      const id = await api.addJob(path);
      selected = id;
    } catch (e) {
      notify(String(e));
    }
  }

  async function pick() {
    if (!engine?.ready) return notify("Install the compression engine first.", "warn");
    const p = await api.pickFolder("Choose a PS5 game dump folder");
    if (p) add(p);
  }

  async function install() {
    installing = true;
    installLog = [];
    try {
      engine = await api.installEngine();
      notify("Compression engine installed.", "ok");
    } catch (e) {
      notify(String(e));
    }
    installing = false;
  }

  async function refreshPs5() {
    try {
      ps5 = await api.ps5Status();
    } catch (e) {
      ps5 = { online: false, error: String(e) };
    }
  }

  const inSetup = $derived(settings && !settings.setupDone);

  async function setupDone(next, pickNow) {
    try {
      await api.saveSettings(next);
    } catch (e) {
      notify(String(e));
    }
    settings = next;
    refreshPs5();
    if (pickNow) setTimeout(pick, 250);
  }

  async function rerunSetup() {
    showHelp = false;
    showSettings = false;
    const next = { ...$state.snapshot(settings), setupDone: false };
    await api.saveSettings(next);
    settings = next;
  }

  async function act(fn, id) {
    try {
      await fn(id);
    } catch (e) {
      notify(String(e));
    }
  }

  onMount(() => {
    const offs = [];
    (async () => {
      settings = await api.getSettings();
      engine = await api.engineStatus();
      setJobs(await api.listJobs());
      offs.push(await api.on("jobs", setJobs));
      offs.push(await api.on("engine-log", (l) => (installLog = [...installLog, l])));
      offs.push(
        await api.onDrop((e) => {
          if (e.type === "enter" || e.type === "over") dragging = true;
          else if (e.type === "leave") dragging = false;
          else if (e.type === "drop") {
            dragging = false;
            if (!engine?.ready) return notify("Install the compression engine first.", "warn");
            for (const p of e.paths ?? []) add(p);
          }
        })
      );
      refreshPs5();
    })();
    // A running upload already proves the link; poll less to keep FTP free.
    const t = setInterval(() => !jobs.some((j) => j.status === "running" && j.stage >= 2) && refreshPs5(), 20000);
    const key = (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "o") {
        e.preventDefault();
        pick();
      } else if ((e.ctrlKey || e.metaKey) && e.key === ",") {
        e.preventDefault();
        showSettings = true;
      } else if (e.key === "F1" || ((e.ctrlKey || e.metaKey) && e.key === "/")) {
        e.preventDefault();
        showHelp = true;
      }
    };
    window.addEventListener("keydown", key);
    return () => {
      clearInterval(t);
      window.removeEventListener("keydown", key);
      offs.forEach((f) => typeof f === "function" && f());
    };
  });
</script>

<div class="app" class:empty={!jobs.length || inSetup} style:--accent={job?.dump.accent ?? "#3d7bff"}>
  <TitleBar {ps5} host={settings?.ps5Host} onSettings={() => (showSettings = true)} onHelp={() => (showHelp = true)} />

  {#if jobs.length && !inSetup}
    <Queue {jobs} {selected} {art} {ps5} onSelect={(id) => (selected = id)} onAdd={pick} />
  {/if}

  <main>
    {#if inSetup}
      <Setup {settings} onDone={setupDone} />
    {:else if job}
      {#key job.id}
        <div class="page" in:fade={{ duration: 220 }}>
          <Hero {job} art={art[job.id]} />
          <div class="content">
            {#if job.warnings.length}
              <div class="notes reveal">
                {#each job.warnings as w}
                  <p><svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M7 1.8 12.6 11.5H1.4L7 1.8Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/><path d="M7 5.6v2.6M7 9.9v.1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>{w}</p>
                {/each}
              </div>
            {/if}
            <div class="reveal" style="animation-delay:.25s"><Pipeline {job} /></div>
            <div class="reveal" style="animation-delay:.32s">
              <Focus
                {job}
                {ahead}
                onCancel={() => act(api.cancelJob, job.id)}
                onResume={() => act(api.resumeJob, job.id)}
                onRemove={() => act(api.removeJob, job.id)}
                onReveal={() => job.imagePath && api.reveal(job.imagePath)}
              />
            </div>
            <div class="reveal" style="animation-delay:.4s"><Activity {job} /></div>
          </div>
        </div>
      {/key}
    {:else}
      <EmptyState {engine} {installing} {installLog} {dragging} onAdd={pick} onInstall={install} />
    {/if}
  </main>

  {#if dragging && jobs.length}
    <div class="dropveil" transition:fade={{ duration: 150 }}>
      <div><b>Drop to add to the queue</b><span>Prospack only reads the dump folder</span></div>
    </div>
  {/if}

  {#if toast}
    <div class="toast {toast.tone}" transition:fly={{ y: 16, duration: 300 }} role="status">{toast.text}</div>
  {/if}

  {#if showSettings && settings}
    <Settings {settings} onClose={() => (showSettings = false)} onSaved={(s) => { settings = s; refreshPs5(); }} onRerunSetup={rerunSetup} />
  {/if}

  {#if showHelp}
    <Help onClose={() => (showHelp = false)} onRerunSetup={rerunSetup} />
  {/if}
</div>

<style>
  .app { display: grid; grid-template-columns: 272px 1fr; grid-template-rows: 48px 1fr; height: 100vh; }
  .app.empty { grid-template-columns: 1fr; }
  .app > :global(header) { grid-column: 1 / -1; }
  main { min-width: 0; min-height: 0; overflow-y: auto; overflow-x: hidden; }
  .page { min-height: 100%; }
  .content { padding: 4px 40px 48px; display: grid; gap: 24px; max-width: 1240px; }
  .notes { display: flex; flex-direction: column; gap: 6px; }
  .notes p {
    display: flex; gap: 10px; align-items: flex-start; font-size: 12.5px; color: var(--warn);
    background: color-mix(in srgb, var(--warn) 7%, transparent); border: 1px solid color-mix(in srgb, var(--warn) 18%, transparent);
    padding: 9px 12px; border-radius: 10px;
  }
  .notes svg { flex: none; margin-top: 2px; }

  .dropveil {
    position: fixed; inset: 48px 0 0 0; z-index: 30; display: grid; place-items: center;
    background: color-mix(in srgb, var(--bg) 80%, transparent); backdrop-filter: blur(6px);
  }
  .dropveil div {
    border: 1px dashed color-mix(in srgb, var(--accent) 70%, transparent); border-radius: 18px; padding: 48px 64px;
    display: flex; flex-direction: column; align-items: center; gap: 6px;
  }
  .dropveil b { font-weight: 500; font-size: 22px; letter-spacing: -0.02em; }
  .dropveil span { color: var(--muted); }

  .toast {
    position: fixed; left: 50%; bottom: 24px; transform: translateX(-50%); z-index: 50; max-width: min(640px, 90vw);
    background: var(--raised); border: 1px solid var(--line-strong); border-radius: 10px; padding: 11px 16px;
    font-size: 13px; box-shadow: 0 20px 40px rgba(0, 0, 0, 0.4); user-select: text;
  }
  .toast.err { border-color: color-mix(in srgb, var(--err) 45%, transparent); }
  .toast.ok { border-color: color-mix(in srgb, var(--ok) 45%, transparent); }
  .toast.warn { border-color: color-mix(in srgb, var(--warn) 45%, transparent); }

  @media (max-width: 900px) {
    .app { grid-template-columns: 1fr; grid-template-rows: 48px auto 1fr; }
    .content { padding: 4px 16px 32px; }
  }
</style>
