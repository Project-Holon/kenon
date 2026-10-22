<script lang="ts">
  import { onMount } from "svelte";
  import { debounced } from "../lib/cache.svelte";
  import { modCheckSettings, modGetSettings, modSaveSettings, type Diagnostic, type ModSettings } from "../lib/modApi";
  import { play } from "../lib/sfx";
  import ModOptions from "./ModOptions.svelte";

  let { readOnly, onSaved }: { readOnly: boolean; onSaved: () => void } = $props();

  const blank: ModSettings = {
    name: "",
    order: 0,
    api: null,
    port: null,
    code: null,
    state: null,
    settings: null,
    multiplayer: null,
    tables: [],
  };
  let form = $state<ModSettings>({ ...blank });
  let original = $state<ModSettings>({ ...blank });
  let apiVersion = $state(0);
  let problems = $state<Diagnostic[]>([]);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let loaded = $state(false);
  let tab = $state<"general" | "options">("general");

  // The form as the file keeps it: an empty box is no value.
  const clean = (s: ModSettings): ModSettings => {
    const text = (v: string | null) => (v?.trim() ? v.trim() : null);
    const code = text(s.code);
    return {
      name: s.name.trim(),
      order: Number.isFinite(s.order) ? Math.trunc(s.order) : 0,
      api: code ? s.api : null,
      port: text(s.port),
      code,
      state: text(s.state),
      settings: text(s.settings),
      multiplayer: text(s.multiplayer),
      tables: s.tables,
    };
  };

  async function load() {
    try {
      const [settings, , api] = await modGetSettings();
      form = { ...settings };
      original = { ...settings };
      apiVersion = api;
      problems = await modCheckSettings(clean(settings));
    } catch (e) {
      error = String(e);
    }
    loaded = true;
  }
  onMount(() => {
    if (!readOnly) load();
  });

  const dirty = $derived(JSON.stringify(clean(form)) !== JSON.stringify(clean(original)));
  const hasCode = $derived(!!form.code?.trim());
  const watched = debounced(() => JSON.stringify(clean(form)), 250);
  let seq = 0;
  $effect(() => {
    const text = watched.value;
    // A value from before the load (or an older edit) is stale: skip it.
    if (readOnly || !loaded || text !== JSON.stringify(clean(form))) return;
    const my = ++seq;
    modCheckSettings(JSON.parse(text))
      .then((d) => my === seq && (problems = d))
      .catch((e) => (error = String(e)));
  });

  // Typing a code file for the first time offers the api the port accepts.
  function codeChanged() {
    if (hasCode && form.api === null) form.api = apiVersion;
  }

  async function save() {
    busy = true;
    error = null;
    try {
      await modSaveSettings(clean(form));
      play("save");
      await load();
      onSaved();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  // Saving options may add the manifest's options-file line.
  function optionsSaved(path: string | null) {
    if (form.settings === original.settings) form.settings = path;
    original.settings = path;
    onSaved();
  }

  const about = (field: string) => problems.filter((p) => p.field === field);
</script>

<div class="settings">
  <header>
    <h2>Mod settings</h2>
    {#if !readOnly}
      <div class="tabs" role="tablist">
        <button role="tab" aria-selected={tab === "general"} class:primary={tab === "general"} onclick={() => (tab = "general")}>General</button>
        <button role="tab" aria-selected={tab === "options"} class:primary={tab === "options"} onclick={() => (tab = "options")}>Options</button>
      </div>
    {/if}
    <div class="spacer"></div>
    {#if !readOnly && tab === "general"}
      <button class="quiet" disabled={!dirty || busy} onclick={() => (form = { ...original })}>Undo</button>
      <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
    {/if}
  </header>

  {#if readOnly}
    <p class="muted pad">Open a project to edit its settings.</p>
  {:else}
    <div class="scroll" hidden={tab !== "general"}>
      {#if error}<p class="bad">{error}</p>{/if}
      <form onsubmit={(e) => (e.preventDefault(), dirty && !busy && save())}>
        <label for="mod-name">Name</label>
        <div class="field">
          <input id="mod-name" bind:value={form.name} />
          <p class="hint muted">Shown in the game's mod menu.</p>
          {#each about("name") as p}<p class="bad">{p.message}</p>{/each}
        </div>

        <label for="mod-order">Load order</label>
        <div class="field">
          <input id="mod-order" type="number" step="1" bind:value={form.order} />
          <p class="hint muted">Lower loads first. When two mods change the same file, the later one wins.</p>
          {#each about("order") as p}<p class="bad">{p.message}</p>{/each}
        </div>

        <label for="mod-code">Code file</label>
        <div class="field">
          <input
            id="mod-code"
            placeholder="none"
            value={form.code ?? ""}
            oninput={(e) => ((form.code = e.currentTarget.value), codeChanged())}
          />
          <p class="hint muted">The mod's compiled code, as a path inside the mod folder.</p>
          {#each about("code") as p}<p class={p.severity === "error" ? "bad" : "warn"}>{p.message}</p>{/each}
        </div>

        {#if hasCode}
          <label for="mod-api">API version</label>
          <div class="field">
            <input id="mod-api" type="number" min="0" step="1" bind:value={form.api} />
            <p class="hint muted">The interface version the code is built for. The game accepts 3 and 4. Use 4 for WebAssembly code.</p>
            {#each about("api") as p}<p class="bad">{p.message}</p>{/each}
          </div>
        {/if}

        <label for="mod-port">Game version</label>
        <div class="field">
          <input id="mod-port" placeholder="none" bind:value={form.port} />
          <p class="hint muted">The game version the hooks are built for. Hooks need it.</p>
          {#each about("port") as p}<p class="warn">{p.message}</p>{/each}
        </div>

        <label for="mod-state">Save name</label>
        <div class="field">
          <input id="mod-state" placeholder="made from the name" bind:value={form.state} />
          <p class="hint muted">The folder name for the mod's saved data.</p>
          {#each about("state") as p}<p class="bad">{p.message}</p>{/each}
        </div>

        <label for="mod-multiplayer">Multiplayer version</label>
        <div class="field">
          <input
            id="mod-multiplayer"
            type="number"
            min="1"
            step="1"
            placeholder="none"
            value={form.multiplayer ?? ""}
            oninput={(e) => (form.multiplayer = e.currentTarget.value)}
          />
          <p class="hint muted">Players can start together only when their copies of this mod have the same number. Leave empty if the mod has no multiplayer part.</p>
          {#each about("multiplayer") as p}<p class={p.severity === "error" ? "bad" : "warn"}>{p.message}</p>{/each}
        </div>

        <label for="mod-settings">Options file</label>
        <div class="field">
          <input id="mod-settings" placeholder="none" bind:value={form.settings} />
          <p class="hint muted">A file in the mod folder that lists the mod's options for the game's settings screen.</p>
          {#each about("settings") as p}<p class="bad">{p.message}</p>{/each}
        </div>

        {#if form.tables.length}
          <span class="label">Tables</span>
          <div class="field">
            <p>{form.tables.map((t) => t.name || "(no name)").join(", ")}</p>
            <p class="hint muted">Edit them in Tables.</p>
            {#each about("tables") as p}<p class={p.severity === "error" ? "bad" : "warn"}>{p.message}</p>{/each}
          </div>
        {/if}
      </form>
    </div>
    <div class="pane" hidden={tab !== "options"}>
      <ModOptions modName={form.name} onSaved={optionsSaved} />
    </div>
  {/if}
</div>

<style>
  .settings {
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
  .tabs {
    display: flex;
    gap: 0.3rem;
  }
  .pane {
    min-height: 0;
    min-width: 0;
  }
  .pane[hidden],
  .scroll[hidden] {
    display: none;
  }
  .scroll {
    overflow-y: auto;
    padding: 1rem;
  }
  .pad {
    padding: 1rem;
  }
  form {
    display: grid;
    grid-template-columns: 9rem minmax(0, 32rem);
    gap: 0.9rem 1rem;
    align-items: start;
  }
  label,
  .label {
    font-weight: 600;
    padding-top: 0.45rem;
  }
  .field {
    display: grid;
    gap: 0.25rem;
  }
  .field input {
    width: 100%;
  }
  .hint {
    font-size: 0.85rem;
  }
  .warn {
    color: var(--gold);
  }
</style>
