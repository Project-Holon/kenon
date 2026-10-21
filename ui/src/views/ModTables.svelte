<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import CodeEditor from "../lib/CodeEditor.svelte";
  import ModDocs from "../lib/ModDocs.svelte";
  import { debounced } from "../lib/cache.svelte";
  import { TABLE_LANGUAGE, setTables, type Marker } from "../lib/monaco";
  import {
    modCheckTable,
    modDeleteTable,
    modListTables,
    modNewTable,
    modReadTable,
    modRenameTable,
    modSaveTable,
    type Diagnostic,
    type TableEntry,
  } from "../lib/modApi";
  import { play } from "../lib/sfx";

  let { readOnly, onSaved }: { readOnly: boolean; onSaved: () => void } = $props();

  let entries = $state<TableEntry[]>([]);
  let name = $state<string | null>(null);
  let text = $state("");
  let original = $state("");
  let problems = $state<Diagnostic[]>([]);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);
  // Unsaved edits of tables left behind.
  const drafts = new SvelteMap<string, string>();

  let creating = $state(false);
  let newName = $state("");
  let renaming = $state<string | null>(null);
  let confirmDelete = $state(false);
  let docs = $state(false);

  const dirty = $derived(name !== null && text !== original);
  const current = $derived(entries.find((e) => e.name === name));

  async function refresh() {
    entries = await modListTables();
    setTables(entries.map((e) => e.name));
  }

  onMount(async () => {
    if (readOnly) return;
    try {
      await refresh();
      if (entries.length) await pick(entries[0].name, false);
    } catch (e) {
      error = String(e);
    }
  });

  async function pick(next: string, sound = true) {
    if (next === name) return;
    if (sound) play("select");
    if (name !== null && dirty) drafts.set(name, text);
    else if (name !== null) drafts.delete(name);
    error = null;
    notice = null;
    confirmDelete = false;
    renaming = null;
    try {
      const disk = (await modReadTable(next).catch(() => "")) as string;
      name = next;
      original = disk;
      text = drafts.get(next) ?? disk;
      drafts.delete(next);
    } catch (e) {
      error = String(e);
    }
  }

  const watched = debounced(() => text, 250);
  let seq = 0;
  $effect(() => {
    const t = watched.value;
    if (readOnly || name === null) return;
    const my = ++seq;
    modCheckTable(t)
      .then((d) => my === seq && (problems = d))
      .catch((e) => (error = String(e)));
  });

  const markers = $derived<Marker[]>(
    problems
      .filter((d) => d.line > 0)
      .map((d) => ({ line: d.line, start: d.start, end: d.end, severity: d.severity, message: d.message })),
  );

  async function save() {
    if (name === null || !dirty || busy) return;
    busy = true;
    error = null;
    notice = null;
    try {
      problems = await modSaveTable(name, text);
      original = text;
      drafts.delete(name);
      await refresh();
      play("save");
      onSaved();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  async function create() {
    error = null;
    notice = null;
    try {
      const made = await modNewTable(newName.trim());
      creating = false;
      newName = "";
      await refresh();
      await pick(made.name);
      onSaved();
    } catch (e) {
      error = String(e);
    }
  }

  async function rename() {
    if (name === null || renaming === null) return;
    error = null;
    notice = null;
    try {
      const was = name;
      const [to, reads] = await modRenameTable(was, renaming.trim());
      if (drafts.has(was)) {
        drafts.set(to.name, drafts.get(was)!);
        drafts.delete(was);
      }
      renaming = null;
      name = to.name;
      await refresh();
      if (reads > 0) notice = `${reads} ${reads === 1 ? "read" : "reads"} in rules changed to the new name.`;
      onSaved();
    } catch (e) {
      error = String(e);
    }
  }

  async function remove() {
    if (name === null) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    error = null;
    notice = null;
    try {
      await modDeleteTable(name);
      drafts.delete(name);
      name = null;
      text = original = "";
      problems = [];
      confirmDelete = false;
      await refresh();
      if (entries.length) await pick(entries[0].name, false);
      play("cancel");
      onSaved();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="tables">
  <aside class="files">
    <div class="head">
      <h3>Tables</h3>
      <div class="spacer"></div>
      <button class="quiet" title="How tables work, and examples" onclick={() => (docs = true)}>Docs</button>
      {#if !readOnly}<button onclick={() => (creating = !creating)}>New</button>{/if}
    </div>
    {#if creating}
      <form
        class="new"
        onsubmit={(e) => {
          e.preventDefault();
          create();
        }}
      >
        <input placeholder="Name" aria-label="Table name" bind:value={newName} />
        <button class="primary" disabled={!newName.trim()}>Create</button>
      </form>
    {/if}
    <div class="scroll">
      {#each entries as e (e.name)}
        <button class="row-item" aria-current={e.name === name} onclick={() => pick(e.name)} title={e.path}>
          <span class="label">{e.name}</span>
          {#if e.name === name ? dirty : drafts.has(e.name)}
            <span class="tag changed" title="Not saved yet">edited</span>
          {:else if !e.exists}
            <span class="tag" title="The manifest names this file, but it is not there">missing</span>
          {/if}
        </button>
      {:else}
        <p class="muted pad">{readOnly ? "Open a project to edit its tables." : "No tables yet."}</p>
      {/each}
    </div>
  </aside>

  <section class="work">
    {#if name !== null}
      <div class="bar">
        {#if renaming !== null}
          <input
            aria-label="New table name"
            bind:value={renaming}
            onkeydown={(e) => (e.key === "Enter" ? rename() : e.key === "Escape" && (renaming = null))}
          />
          <button class="primary" onclick={rename}>Rename</button>
          <button class="quiet" onclick={() => (renaming = null)}>Cancel</button>
        {:else}
          <code class="where">table.{name}[key]</code>
          <span class="muted file">{current?.path ?? ""}</span>
          <div class="spacer"></div>
          <button class="quiet" onclick={() => (renaming = name)}>Rename</button>
          <button class="quiet" class:bad={confirmDelete} onclick={remove} onblur={() => (confirmDelete = false)}>
            {confirmDelete ? "Delete for good?" : "Delete"}
          </button>
          <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
        {/if}
      </div>
      <div class="editor">
        <CodeEditor bind:value={text} language={TABLE_LANGUAGE} {markers} onSave={save} label="Table" />
      </div>
      <div class="problems">
        {#if error}<p class="bad">{error}</p>{/if}
        {#if notice}<p class="muted">{notice}</p>{/if}
        {#each problems as d}
          <p class={d.severity === "error" ? "bad" : "warn"}>{d.line ? `Line ${d.line}: ` : ""}{d.message}</p>
        {:else}
          <p class="muted">No problems.</p>
        {/each}
      </div>
    {:else}
      {#if error}<p class="bad pad">{error}</p>{/if}
      <p class="muted pad">{readOnly ? "" : "Choose a table, or make a new one."}</p>
    {/if}
  </section>
</div>

{#if docs}<ModDocs select="page:tables" onClose={() => (docs = false)} />{/if}

<style>
  .tables {
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
  .bar {
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
    overflow-y: auto;
    min-height: 0;
  }
  .work {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    gap: 0.5rem;
    padding: 0.6rem 1rem;
    min-width: 0;
    min-height: 0;
  }
  .where,
  .file {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file {
    font-size: 0.85rem;
  }
  .editor {
    min-height: 0;
  }
  .problems {
    max-height: 7rem;
    overflow-y: auto;
    font-size: 0.9rem;
  }
  .problems p,
  .pad {
    margin: 0;
  }
  .warn {
    color: var(--gold);
  }
</style>
