<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import CodeEditor from "../lib/CodeEditor.svelte";
  import ModDocs from "../lib/ModDocs.svelte";
  import SearchSelect from "morphe/SearchSelect.svelte";
  import { debounced } from "morphe/cache.svelte";
  import { RULE_LANGUAGE, setRuleNames, setTables, type Marker } from "../lib/monaco";
  import {
    modCheckRule,
    modDeleteRule,
    modListRules,
    modListTables,
    modNewRule,
    modReadRule,
    modRenameRule,
    modRuleNames,
    modSaveRule,
    type Diagnostic,
    type RuleEntry,
  } from "../lib/modApi";
  import { play } from "../lib/sfx";

  let { readOnly, onSaved }: { readOnly: boolean; onSaved: () => void } = $props();

  let entries = $state<RuleEntry[]>([]);
  let events = $state<string[]>([]);
  let path = $state<string | null>(null);
  let text = $state("");
  let original = $state("");
  let problems = $state<Diagnostic[]>([]);
  let error = $state<string | null>(null);
  let busy = $state(false);
  // Unsaved edits of files left behind.
  const drafts = new SvelteMap<string, string>();

  let creating = $state(false);
  let newName = $state("");
  let newEvent = $state(0);
  let renaming = $state<string | null>(null);
  let confirmDelete = $state(false);
  let docs = $state(false);

  const dirty = $derived(path !== null && text !== original);
  const current = $derived(entries.find((e) => e.path === path));
  const docsEvent = $derived(current?.fileEvent ?? current?.bound ?? null);

  async function refresh() {
    entries = await modListRules();
  }

  onMount(async () => {
    if (readOnly) return;
    try {
      const names = await modRuleNames();
      setRuleNames(names);
      events = names.events.map((e) => e.name);
      await refresh();
      setTables((await modListTables()).map((t) => t.name));
      if (entries.length) await pick(entries[0].path, false);
    } catch (e) {
      error = String(e);
    }
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
      const disk = (await modReadRule(next).catch(() => "")) as string;
      path = next;
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
    // A value from another file (or an older edit) is stale: skip it.
    if (readOnly || path === null || t !== text) return;
    const my = ++seq;
    modCheckRule(t)
      .then((d) => my === seq && (problems = d))
      .catch((e) => (error = String(e)));
  });

  const markers = $derived<Marker[]>(
    problems
      .filter((d) => d.line > 0)
      .map((d) => ({ line: d.line, start: d.start, end: d.end, severity: d.severity, message: d.message })),
  );

  async function save() {
    if (path === null || !dirty || busy) return;
    busy = true;
    error = null;
    try {
      problems = await modSaveRule(path, text);
      original = text;
      drafts.delete(path);
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
    try {
      const made = await modNewRule(newName.trim(), events[newEvent]);
      creating = false;
      newName = "";
      await refresh();
      await pick(made);
      onSaved();
    } catch (e) {
      error = String(e);
    }
  }

  async function rename() {
    if (path === null || renaming === null) return;
    error = null;
    try {
      const was = path;
      const to = await modRenameRule(was, renaming.includes("/") ? renaming : `rules/${renaming}`);
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
      await modDeleteRule(path);
      drafts.delete(path);
      path = null;
      text = original = "";
      problems = [];
      confirmDelete = false;
      await refresh();
      if (entries.length) await pick(entries[0].path, false);
      play("cancel");
      onSaved();
    } catch (e) {
      error = String(e);
    }
  }

  const shortName = (p: string) => p.replace(/^rules\//, "");
</script>

<div class="rules">
  <aside class="files">
    <div class="head">
      <h3>Rules</h3>
      <div class="spacer"></div>
      <button title="Events, payloads, state fields and examples" onclick={() => (docs = true)}>Docs</button>
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
        <input placeholder="Name" aria-label="Rule name" bind:value={newName} />
        <SearchSelect bind:value={newEvent} options={events} label="Event" />
        <button class="primary" disabled={!newName.trim()}>Create</button>
      </form>
    {/if}
    <div class="scroll">
      {#each entries as e (e.path)}
        <button class="row-item" aria-current={e.path === path} onclick={() => pick(e.path)} title={e.path}>
          <span class="label">{shortName(e.path)}</span>
          {#if e.path === path ? dirty : drafts.has(e.path)}
            <span class="tag changed" title="Not saved yet">edited</span>
          {:else if !e.exists}
            <span class="tag" title="The manifest names this file, but it is not there">missing</span>
          {/if}
        </button>
        <span class="event muted">{e.fileEvent ?? e.bound ?? "no event"}{e.bound ? "" : " (not bound)"}</span>
      {:else}
        <p class="muted pad">{readOnly ? "Open a project to edit its rules." : "No rules yet."}</p>
      {/each}
    </div>
  </aside>

  <section class="work">
    {#if path !== null}
      <div class="bar">
        {#if renaming !== null}
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
          <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
        {/if}
      </div>
      <div class="editor">
        <CodeEditor bind:value={text} language={RULE_LANGUAGE} {markers} onSave={save} label="Rule" />
      </div>
      <div class="problems">
        {#if error}<p class="bad">{error}</p>{/if}
        {#each problems as d}
          <p class={d.severity === "error" ? "bad" : "warn"}>{d.line ? `Line ${d.line}: ` : ""}{d.message}</p>
        {:else}
          <p class="muted">No problems.</p>
        {/each}
      </div>
    {:else}
      {#if error}<p class="bad pad">{error}</p>{/if}
      <p class="muted pad">{readOnly ? "" : "Choose a rule, or make a new one."}</p>
    {/if}
  </section>
</div>

{#if docs}<ModDocs select={docsEvent ? `event:${docsEvent}` : "page:overview"} onClose={() => (docs = false)} />{/if}

<style>
  .rules {
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
  .event {
    display: block;
    padding: 0 0.7rem 0.3rem;
    font-size: 0.8rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .work {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    gap: 0.5rem;
    padding: 0.6rem 1rem;
    min-width: 0;
    min-height: 0;
  }
  .where {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
