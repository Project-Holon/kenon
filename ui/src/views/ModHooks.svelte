<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import CodeEditor from "../lib/CodeEditor.svelte";
  import ModDocs from "../lib/ModDocs.svelte";
  import { debounced } from "morphe/cache.svelte";
  import { HOOK_LANGUAGE, setDeclared, type Marker } from "../lib/monaco";
  import {
    modCheckHooks,
    modDeclared,
    modGetHooks,
    modGetPaths,
    modSaveHooks,
    modSetPaths,
    type HooksCheck,
    type ModPaths,
  } from "../lib/modApi";
  import { play } from "../lib/sfx";

  let { readOnly, onSaved }: { readOnly: boolean; onSaved: () => void } = $props();

  let text = $state("");
  let original = $state("");
  let check = $state<HooksCheck>({ diagnostics: [], rows: [] });
  let paths = $state<ModPaths>({ modsFolder: null, declaredSet: null, declaredFound: null });
  let error = $state<string | null>(null);
  let busy = $state(false);
  let loaded = $state(false);
  let docs = $state(false);

  const dirty = $derived(text !== original);
  const watched = debounced(() => text, 250);
  let seq = 0;
  // Asked again when the declared set changes.
  let setVersion = $state(0);

  async function load() {
    try {
      text = original = await modGetHooks();
      paths = await modGetPaths();
      setDeclared(await modDeclared());
    } catch (e) {
      error = String(e);
    }
    loaded = true;
  }
  onMount(() => {
    if (!readOnly) load();
  });

  $effect(() => {
    const t = watched.value;
    void setVersion;
    // A value from before the load (or an older edit) is stale: skip it.
    if (readOnly || !loaded || t !== text) return;
    const my = ++seq;
    modCheckHooks(t)
      .then((c) => my === seq && (check = c))
      .catch((e) => (error = String(e)));
  });

  const markers = $derived<Marker[]>(
    check.diagnostics
      .filter((d) => d.line > 0)
      .map((d) => ({ line: d.line, start: d.start, end: d.end, severity: d.severity, message: d.message })),
  );

  async function save() {
    if (!dirty || busy) return;
    busy = true;
    error = null;
    try {
      await modSaveHooks(text);
      original = text;
      play("save");
      onSaved();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  async function chooseSet() {
    const picked = await open({
      multiple: false,
      title: "Choose the list of hookable functions",
      filters: [{ name: "Text", extensions: ["txt"] }],
    });
    if (typeof picked !== "string") return;
    await setPaths(picked);
  }

  async function setPaths(declaredSet: string | null) {
    try {
      paths = await modSetPaths(paths.modsFolder, declaredSet);
      setDeclared(await modDeclared());
      setVersion++;
    } catch (e) {
      error = String(e);
    }
  }

  const listed = $derived(
    paths.declaredFound === null
      ? "No list found"
      : `${paths.declaredFound} functions${paths.declaredSet ? "" : " (found beside the mods folder)"}`,
  );
</script>

<div class="hooks">
  <header>
    <h2>Hooks</h2>
    <span class="muted">{listed}</span>
    <div class="spacer"></div>
    <button title="The hookable functions and how hooks work" onclick={() => (docs = true)}>Docs</button>
    {#if !readOnly}
      <button
        title="The list is eidos's mod/declared_set.txt. It names the game functions a hook can reach."
        onclick={chooseSet}>Choose list</button
      >
      {#if paths.declaredSet}<button class="quiet" onclick={() => setPaths(null)}>Clear</button>{/if}
      <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
    {/if}
  </header>

  {#if readOnly}
    <p class="muted pad">Open a project to edit its hooks.</p>
  {:else}
    <div class="body">
      <div class="editor">
        <CodeEditor bind:value={text} language={HOOK_LANGUAGE} {markers} onSave={save} label="Hook lines" />
      </div>
      <aside>
        {#if error}<p class="bad">{error}</p>{/if}
        <p class="muted help">
          One game function per line: <code>region:address mode symbol</code>. The mode is <code>wrap</code>
          (calls the game's function) or <code>replace</code>. The symbol is a function in the mod's code file.
          Optional: <code>sig="int (int)"</code> and <code>switch=name</code>.
        </p>
        <h3>Problems</h3>
        <div class="list">
          {#each check.diagnostics as d}
            <p class={d.severity === "error" ? "bad" : "warn"}>{d.line ? `Line ${d.line}: ` : ""}{d.message}</p>
          {:else}
            <p class="muted">None.</p>
          {/each}
        </div>
        {#if check.rows.length}
          <h3>Functions</h3>
          <div class="list">
            {#each check.rows as r}
              <p>
                <span class="muted">Line {r.line}</span>
                <code>{r.function}</code>
                {#if r.aliases.length}<span class="muted">also named {r.aliases.join(", ")}</span>{/if}
                {#if r.events.length}<span class="muted">{r.events.join(", ")}</span>{/if}
              </p>
            {/each}
          </div>
        {/if}
      </aside>
    </div>
  {/if}
</div>

{#if docs}<ModDocs select="page:hooks" onClose={() => (docs = false)} />{/if}

<style>
  .hooks {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    height: 100%;
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .spacer {
    flex: 1;
  }
  .pad {
    padding: 1rem;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 18rem;
    gap: 0.8rem;
    padding: 0.8rem 1rem;
    min-height: 0;
  }
  .editor {
    min-height: 0;
  }
  aside {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    min-width: 0;
  }
  .help {
    font-size: 0.85rem;
  }
  .list {
    display: grid;
    gap: 0.25rem;
    font-size: 0.9rem;
    overflow-wrap: anywhere;
  }
  .list p {
    margin: 0;
  }
  .warn {
    color: var(--gold);
  }
</style>
