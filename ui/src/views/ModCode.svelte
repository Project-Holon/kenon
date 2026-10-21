<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import CodeEditor from "../lib/CodeEditor.svelte";
  import type { Marker } from "../lib/monaco";
  import { C_LANGUAGE, ensureC, setCodeApi } from "../lib/monacoC";
  import {
    buildCodeMod,
    fetchZig,
    modCodeApi,
    modCodeDelete,
    modCodeFiles,
    modCodeNew,
    modCodeRead,
    modCodeRename,
    modCodeWrite,
    newCodeMod,
    setZigPath,
    zigStatus,
    type BuildResult,
    type SourceFile,
    type ZigProgress,
    type ZigStatus,
  } from "../lib/modApi";
  import { play } from "../lib/sfx";

  let { readOnly, onSaved }: { readOnly: boolean; onSaved: () => void } = $props();

  let files = $state<SourceFile[]>([]);
  let path = $state<string | null>(null);
  let text = $state("");
  let original = $state("");
  let error = $state<string | null>(null);
  let loaded = $state(false);
  let ready = $state(false);
  // Unsaved edits of files left behind.
  const drafts = new SvelteMap<string, string>();

  let creating = $state(false);
  let newName = $state("");
  let renaming = $state<string | null>(null);
  let confirmDelete = $state(false);

  let zig = $state<ZigStatus | null>(null);
  let progress = $state<ZigProgress | null>(null);
  let fetching = $state(false);
  let building = $state(false);
  let result = $state<BuildResult | null>(null);

  const dirty = $derived(path !== null && text !== original);
  const sources = $derived(files.filter((f) => f.path.startsWith("src/")));
  const headers = $derived(files.filter((f) => f.path.startsWith("include/")));

  async function refresh() {
    files = await modCodeFiles();
  }

  onMount(() => {
    if (readOnly) return;
    let gone = false;
    let unlisten: (() => void) | undefined;
    listen<ZigProgress>("zig-progress", (e) => (progress = e.payload)).then((u) => {
      if (gone) u();
      else unlisten = u;
    });
    (async () => {
      try {
        await ensureC();
        zig = await zigStatus();
        await refresh();
        setCodeApi(await modCodeApi());
        if (files.length) await pick(files.find((f) => f.path === "src/mod.c")?.path ?? files[0].path, false);
      } catch (e) {
        error = String(e);
      }
      ready = true;
      loaded = true;
    })();
    return () => {
      gone = true;
      unlisten?.();
    };
  });

  async function pick(next: string, sound = true) {
    if (next === path) return;
    if (sound) play("select");
    if (path !== null && dirty) drafts.set(path, text);
    else if (path !== null) drafts.delete(path);
    error = null;
    confirmDelete = false;
    renaming = null;
    try {
      const disk = await modCodeRead(next);
      path = next;
      original = disk;
      text = drafts.get(next) ?? disk;
      drafts.delete(next);
    } catch (e) {
      error = String(e);
    }
  }

  async function start() {
    error = null;
    try {
      await newCodeMod();
      await refresh();
      setCodeApi(await modCodeApi());
      await pick("src/mod.c", false);
      onSaved();
    } catch (e) {
      error = String(e);
    }
  }

  /// Writes every edited file.
  async function saveAll() {
    if (path !== null && dirty) {
      await modCodeWrite(path, text);
      original = text;
    }
    for (const [p, t] of [...drafts]) {
      await modCodeWrite(p, t);
      drafts.delete(p);
    }
  }

  async function save() {
    if (path === null || !dirty) return;
    error = null;
    try {
      await modCodeWrite(path, text);
      original = text;
      play("save");
      onSaved();
    } catch (e) {
      error = String(e);
      play("cancel");
    }
  }

  async function create() {
    error = null;
    let name = newName.trim();
    if (!name.includes("/")) name = `${name.endsWith(".h") ? "include" : "src"}/${name}`;
    if (!/\.[ch]$/.test(name)) name += name.startsWith("include/") ? ".h" : ".c";
    try {
      await modCodeNew(name);
      creating = false;
      newName = "";
      await refresh();
      await pick(name);
      onSaved();
    } catch (e) {
      error = String(e);
    }
  }

  async function rename() {
    if (path === null || renaming === null) return;
    error = null;
    const was = path;
    const dir = was.startsWith("include/") ? "include" : "src";
    const to = renaming.includes("/") ? renaming : `${dir}/${renaming}`;
    try {
      if (dirty) await modCodeWrite(was, text);
      original = text;
      await modCodeRename(was, to);
      if (drafts.has(was)) {
        drafts.set(to, drafts.get(was)!);
        drafts.delete(was);
      }
      renaming = null;
      path = to;
      await refresh();
      onSaved();
    } catch (e) {
      error = String(e);
    }
  }

  async function remove() {
    if (path === null) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    error = null;
    try {
      await modCodeDelete(path);
      drafts.delete(path);
      path = null;
      text = original = "";
      confirmDelete = false;
      await refresh();
      if (files.length) await pick(files[0].path, false);
      play("cancel");
      onSaved();
    } catch (e) {
      error = String(e);
    }
  }

  async function download() {
    error = null;
    progress = null;
    fetching = true;
    try {
      zig = await fetchZig();
    } catch (e) {
      error = String(e);
    } finally {
      fetching = false;
      progress = null;
    }
  }

  async function chooseZig() {
    const picked = await open({ multiple: false, title: "Choose your Zig compiler" });
    if (typeof picked !== "string") return;
    error = null;
    try {
      zig = await setZigPath(picked);
    } catch (e) {
      error = String(e);
    }
  }

  async function useDownloaded() {
    try {
      zig = await setZigPath(null);
    } catch (e) {
      error = String(e);
    }
  }

  async function build() {
    if (building || readOnly || !zig?.path || !files.length) return;
    building = true;
    error = null;
    try {
      await saveAll();
      result = await buildCodeMod();
      play(result.ok ? "save" : "cancel");
      onSaved();
    } catch (e) {
      error = String(e);
    } finally {
      building = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "b") {
      e.preventDefault();
      build();
    }
  }

  function size(bytes: number) {
    return bytes < 1024 ? `${bytes} bytes` : `${(bytes / 1024).toFixed(1)} KB`;
  }

  const summary = $derived.by(() => {
    if (!result) return "";
    const secs = `${(result.millis / 1000).toFixed(1)} s`;
    if (result.ok) return `Built ${result.wasmPath}: ${size(result.size)} in ${secs}`;
    const errors = result.diagnostics.filter((d) => d.severity === "error").length;
    return `Build failed: ${errors} ${errors === 1 ? "error" : "errors"}`;
  });

  // A problem's marker covers the word at its column.
  const markers = $derived<Marker[]>(
    (result?.diagnostics ?? [])
      .filter((d) => d.file === path && d.line > 0 && d.severity !== "note")
      .map((d) => {
        const col = Math.max(d.column, 1);
        const line = text.split("\n")[d.line - 1] ?? "";
        const len = /^\w+/.exec(line.slice(col - 1))?.[0].length ?? 1;
        return { line: d.line, start: col, end: col + len, severity: d.severity as "error" | "warning", message: d.message };
      }),
  );

  const shown = $derived((result?.diagnostics ?? []).filter((d) => d.severity !== "note" || d.file));

  const phases = { download: "Downloading", check: "Checking", unpack: "Unpacking" };
  const shortName = (p: string) => p.replace(/^(src|include)\//, "");
</script>

<svelte:window onkeydown={onKey} />

<div class="code">
  <aside class="files">
    <div class="head">
      <h3>Code</h3>
      <div class="spacer"></div>
      {#if !readOnly && files.length}<button onclick={() => (creating = !creating)}>New</button>{/if}
    </div>
    {#if creating}
      <form
        class="new"
        onsubmit={(e) => {
          e.preventDefault();
          create();
        }}
      >
        <input placeholder="battle.c or util.h" aria-label="File name" bind:value={newName} />
        <button class="primary" disabled={!newName.trim()}>Create</button>
      </form>
    {/if}
    <div class="scroll">
      {#each [["src", sources], ["include", headers]] as const as [dir, list] (dir)}
        {#if list.length}
          <p class="dir muted">{dir}</p>
          {#each list as f (f.path)}
            <button class="row-item" aria-current={f.path === path} onclick={() => pick(f.path)} title={f.path}>
              <span class="label">{shortName(f.path)}</span>
              {#if f.path === path ? dirty : drafts.has(f.path)}
                <span class="tag changed" title="Not saved yet">edited</span>
              {/if}
            </button>
          {/each}
        {/if}
      {/each}
    </div>
    {#if zig?.path}
      <div class="zig muted">
        <span title={zig.path}>Zig {zig.version ?? ""}</span>
        <button class="quiet" onclick={chooseZig}>Change</button>
        {#if zig.chosen}<button class="quiet" onclick={useDownloaded}>Reset</button>{/if}
      </div>
    {/if}
    <p class="muted draft">The interface is a draft until eidos publishes version 4.</p>
  </aside>

  <section class="work">
    {#if readOnly}
      <p class="muted pad">Open a project to write code.</p>
    {:else if !loaded}
      <p class="muted pad">Loading.</p>
    {:else if !files.length}
      <div class="panel">
        <p>This mod has no code. Code mods are written in C and built to one WebAssembly file.</p>
        <div class="row"><button class="primary" onclick={start}>Start a code mod</button></div>
        {#if error}<p class="bad">{error}</p>{/if}
      </div>
    {:else}
      {#if zig && !zig.path}
        <div class="panel">
          <p>
            kenon needs the Zig compiler{zig.downloadMb ? ` (about ${zig.downloadMb} MB)` : ""} to build code mods.
          </p>
          {#if fetching}
            <div class="row">
              <progress max={progress?.total || 1} value={progress?.done ?? 0}></progress>
              <span class="muted">{progress ? phases[progress.phase] : "Starting"}</span>
            </div>
          {:else}
            <div class="row">
              {#if zig.downloadMb}<button class="primary" onclick={download}>Download</button>{/if}
              <button onclick={chooseZig}>Use my own Zig...</button>
            </div>
          {/if}
        </div>
      {/if}
      <div class="bar">
        {#if path === null}
          <span class="muted">Choose a file.</span>
        {:else if renaming !== null}
          <input
            aria-label="New file name"
            bind:value={renaming}
            onkeydown={(e) => (e.key === "Enter" ? rename() : e.key === "Escape" && (renaming = null))}
          />
          <button class="primary" onclick={rename}>Rename</button>
          <button class="quiet" onclick={() => (renaming = null)}>Cancel</button>
        {:else}
          <code class="where">{path}</code>
          <div class="spacer"></div>
          <button class="quiet" onclick={() => (renaming = shortName(path!))}>Rename</button>
          <button class="quiet" class:bad={confirmDelete} onclick={remove} onblur={() => (confirmDelete = false)}>
            {confirmDelete ? "Delete for good?" : "Delete"}
          </button>
          <button disabled={!dirty} onclick={save}>Save</button>
        {/if}
      </div>
      <div class="editor">
        {#if ready && path !== null}
          <CodeEditor bind:value={text} language={C_LANGUAGE} {markers} onSave={save} label="C source" />
        {/if}
      </div>
      <div class="build">
        <button class="primary" disabled={building || !zig?.path} onclick={build} title="Ctrl+B">
          {building ? "Building" : "Build"}
        </button>
        <span class={result && !result.ok ? "bad" : "muted"}>{summary}</span>
        {#if error}<span class="bad">{error}</span>{/if}
      </div>
      {#if shown.length}
        <div class="problems">
          {#each shown as d}
            <p class={d.severity === "error" ? "bad" : d.severity === "warning" ? "warn" : "muted"}>
              {#if d.file && d.line}
                <button class="quiet at" onclick={() => pick(d.file)}>{d.file}:{d.line}</button>
              {:else if d.file}
                <span class="muted">{d.file}</span>
              {/if}
              {d.message}
            </p>
          {/each}
        </div>
      {/if}
    {/if}
  </section>
</div>

<style>
  .code {
    display: grid;
    grid-template-columns: 15rem minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    height: 100%;
  }
  .files {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 0.6rem;
    gap: 0.4rem;
    border-right: 2px solid var(--outline);
  }
  .head,
  .bar,
  .build,
  .row,
  .zig {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .spacer {
    flex: 1;
  }
  .new {
    display: grid;
    gap: 0.4rem;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }
  .dir {
    margin: 0.4rem 0.6rem 0.1rem;
    font-size: 0.8rem;
  }
  .zig {
    font-size: 0.85rem;
  }
  .zig span {
    flex: 1;
  }
  .draft {
    margin: 0;
    font-size: 0.8rem;
  }
  .work {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.6rem 1rem;
    min-width: 0;
    min-height: 0;
  }
  .panel {
    display: grid;
    gap: 0.5rem;
    padding: 0.7rem 0.9rem;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    background: var(--panel);
  }
  .panel p,
  .pad,
  .problems p {
    margin: 0;
  }
  progress {
    flex: 1;
    min-width: 0;
  }
  .where {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .editor {
    flex: 1;
    min-height: 8rem;
  }
  .problems {
    max-height: 8rem;
    overflow-y: auto;
    font-size: 0.9rem;
    overflow-wrap: anywhere;
  }
  .at {
    padding: 0 0.3rem;
    font-family: ui-monospace, Consolas, monospace;
  }
  .warn {
    color: var(--gold);
  }
</style>
