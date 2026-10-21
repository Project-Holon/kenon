<script lang="ts">
  import { onMount } from "svelte";
  import { debounced } from "../lib/cache.svelte";
  import {
    modCheckOptions,
    modReadOptions,
    modSaveOptions,
    type Diagnostic,
    type OptionDecl,
    type OptionsFile,
  } from "../lib/modApi";
  import { play } from "../lib/sfx";

  let { modName, onSaved }: { modName: string; onSaved: (path: string | null) => void } = $props();

  const LIMIT = 4096;
  const kinds = [
    { id: "bool", label: "On or off" },
    { id: "choice", label: "Choice" },
    { id: "int", label: "Number" },
  ] as const;

  let options = $state<OptionDecl[]>([]);
  let original = $state("[]");
  let path = $state<string | null>(null);
  let ignored = $state<Diagnostic[]>([]);
  let problems = $state<Diagnostic[]>([]);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let loaded = $state(false);

  const whole = (list: OptionDecl[]) => JSON.stringify(list);
  const dirty = $derived(whole(options) !== original);

  function take(f: OptionsFile) {
    options = f.options.map((o) => ({ ...o, values: [...o.values] }));
    original = whole(options);
    path = f.path;
    // Lines the game skips are not in the list.
    ignored = f.diagnostics.filter((d) => d.line > 0 && !d.field);
  }

  async function load() {
    try {
      take(await modReadOptions());
    } catch (e) {
      error = String(e);
    }
    loaded = true;
  }
  onMount(load);

  // The list as the file keeps it: numbers are whole.
  const clean = (list: OptionDecl[]): OptionDecl[] =>
    list.map((o) => {
      const n = (v: number) => (Number.isFinite(v) ? Math.trunc(v) : 0);
      return {
        ...o,
        key: o.key.trim(),
        label: o.label.trim(),
        default: n(o.default),
        min: n(o.min),
        max: n(o.max),
        values: o.values.map((v) => v.trim()),
      };
    });

  const watched = debounced(() => JSON.stringify(clean(options)), 200);
  let seq = 0;
  $effect(() => {
    const text = watched.value;
    // A value from before the load (or an older edit) is stale: skip it.
    if (!loaded || text !== JSON.stringify(clean(options))) return;
    const list: OptionDecl[] = JSON.parse(text);
    const my = ++seq;
    modCheckOptions(list)
      .then((d) => my === seq && (problems = d))
      .catch((e) => (error = String(e)));
  });

  const about = (i: number) => problems.filter((p) => p.field === `option:${i}`);
  const blocked = $derived(problems.some((p) => p.severity === "error"));

  function add() {
    let n = options.length + 1;
    while (options.some((o) => o.key === `option${n}`)) n++;
    options.push({
      key: `option${n}`,
      label: "",
      kind: "bool",
      default: 1,
      min: 0,
      max: 10,
      values: [],
      line: 0,
      lead: [],
      comment: "",
      raw: "",
    });
  }

  function remove(i: number) {
    options.splice(i, 1);
  }

  function move(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= options.length) return;
    [options[i], options[j]] = [options[j], options[i]];
  }

  function setKind(o: OptionDecl, kind: OptionDecl["kind"]) {
    o.kind = kind;
    if (kind === "bool") o.default = o.default ? 1 : 0;
    if (kind === "choice") {
      o.default = 0;
      if (!o.values.length) o.values = ["off", "on"];
    }
    if (kind === "int") {
      if (!(o.min <= o.max)) o.max = o.min;
      o.default = Math.min(Math.max(o.default, o.min), o.max);
    }
  }

  function moveValue(o: OptionDecl, i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= o.values.length) return;
    [o.values[i], o.values[j]] = [o.values[j], o.values[i]];
  }

  async function save() {
    busy = true;
    error = null;
    try {
      take(await modSaveOptions(clean(options)));
      play("save");
      onSaved(path);
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  const shown = (o: OptionDecl) => {
    if (o.kind === "bool") return o.default ? "On" : "Off";
    if (o.kind === "choice") return o.values.find((v) => v.trim())?.trim() ?? "";
    return String(Number.isFinite(o.default) ? o.default : 0);
  };
</script>

<div class="options">
  <div class="bar">
    <span class="muted">{path ?? "A new file, options.txt, is made on save."}</span>
    <div class="spacer"></div>
    <button class="quiet" disabled={options.length >= LIMIT} onclick={add}>Add option</button>
    <button class="quiet" disabled={!dirty || busy} onclick={() => take({ path, exists: true, options: JSON.parse(original), diagnostics: [] })}>Undo</button>
    <button class="primary" disabled={!dirty || busy || blocked} onclick={save}>Save</button>
  </div>

  <div class="scroll">
    {#if error}<p class="bad">{error}</p>{/if}
    {#each ignored as p}<p class="bad">Line {p.line}: {p.message}</p>{/each}

    {#if !options.length}
      <p class="muted">No options. Options show on the game's settings screen under the mod's name, and rules read them as setting.key.</p>
    {/if}

    <ol class="list">
      {#each options as o, i}
        <li class="item">
          <div class="head">
            <span class="move">
              <button class="step" disabled={i === 0} onclick={() => move(i, -1)} aria-label="Move up">&#9650;</button>
              <button class="step" disabled={i === options.length - 1} onclick={() => move(i, 1)} aria-label="Move down">&#9660;</button>
            </span>
            <select value={o.kind} aria-label="Kind" onchange={(e) => setKind(o, e.currentTarget.value as OptionDecl["kind"])}>
              {#each kinds as k}<option value={k.id}>{k.label}</option>{/each}
            </select>
            <input class="key" aria-label="Key" placeholder="key" maxlength="63" bind:value={o.key} />
            <input class="label" aria-label="Label" placeholder="Label" maxlength="63" bind:value={o.label} />
            <button class="quiet" onclick={() => remove(i)} aria-label="Remove option">Remove</button>
          </div>

          <div class="body">
            {#if o.kind === "bool"}
              <label class="inline">
                <input type="checkbox" checked={o.default === 1} onchange={(e) => (o.default = e.currentTarget.checked ? 1 : 0)} />
                On by default
              </label>
            {:else if o.kind === "choice"}
              <div class="values">
                {#each o.values as _v, j}
                  <div class="value">
                    <span class="num muted">{j === 0 ? "Default" : j + 1}</span>
                    <input aria-label="Value {j + 1}" maxlength="31" bind:value={o.values[j]} />
                    <button class="step" disabled={j === 0} onclick={() => moveValue(o, j, -1)} aria-label="Move value up">&#9650;</button>
                    <button class="step" disabled={j === o.values.length - 1} onclick={() => moveValue(o, j, 1)} aria-label="Move value down">&#9660;</button>
                    <button class="step" onclick={() => o.values.splice(j, 1)} aria-label="Remove value">-</button>
                  </div>
                {/each}
                <button class="quiet add" disabled={o.values.length >= 1024} onclick={() => o.values.push("")}>Add value</button>
              </div>
            {:else}
              <div class="ints">
                <label>Lowest <input type="number" step="1" bind:value={o.min} /></label>
                <label>Highest <input type="number" step="1" bind:value={o.max} /></label>
                <label>Default <input type="number" step="1" bind:value={o.default} /></label>
              </div>
            {/if}
          </div>
          {#each about(i) as p}<p class={p.severity === "error" ? "bad" : "warn"}>{p.message}</p>{/each}
        </li>
      {/each}
    </ol>

    {#if options.length}
      <h3>Settings screen</h3>
      <div class="preview" aria-label="Preview">
        <p class="mod">{modName || "Mod"}</p>
        {#each options as o}
          <div class="prow">
            <span>{o.label.trim() || "(no label)"}</span>
            <span class="val">
              {#if o.kind !== "bool"}<span class="arrow">&lsaquo;</span>{/if}{shown(o)}{#if o.kind !== "bool"}<span class="arrow">&rsaquo;</span>{/if}
            </span>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .options {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    height: 100%;
    min-width: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.4rem 1rem;
    border-bottom: 2px solid var(--outline);
    min-width: 0;
  }
  .bar .muted {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .spacer {
    flex: 1;
  }
  .scroll {
    overflow-y: auto;
    padding: 1rem;
    min-width: 0;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.7rem;
    max-width: 44rem;
  }
  .item {
    background: var(--panel);
    border: 2px solid var(--outline);
    border-radius: var(--radius);
    padding: 0.6rem 0.7rem;
    display: grid;
    gap: 0.5rem;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }
  .move {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .step {
    padding: 0 0.4rem;
    line-height: 1.3;
    font-size: 0.75rem;
  }
  .key {
    flex: 1 1 8rem;
    min-width: 0;
  }
  .label {
    flex: 2 1 10rem;
    min-width: 0;
  }
  .body {
    padding-left: 2rem;
    min-width: 0;
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .values {
    display: grid;
    gap: 0.35rem;
    justify-items: start;
  }
  .value {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }
  .num {
    width: 3.6rem;
    font-size: 0.85rem;
  }
  .ints {
    display: flex;
    flex-wrap: wrap;
    gap: 0.7rem;
  }
  .ints label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .ints input {
    width: 6.5rem;
  }
  .warn {
    color: var(--gold);
    margin: 0;
  }
  .item .bad {
    margin: 0;
  }
  h3 {
    margin: 1.2rem 0 0.5rem;
  }
  .preview {
    max-width: 26rem;
    background: var(--panel-2);
    border: 2px solid var(--outline);
    border-radius: var(--radius);
    padding: 0.5rem 0.7rem;
    display: grid;
    gap: 0.3rem;
  }
  .mod {
    margin: 0;
    font-weight: 700;
    color: var(--muted);
  }
  .prow {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.2rem 0.4rem;
    border-radius: var(--radius-s);
  }
  .prow:nth-child(odd) {
    background: var(--panel-3);
  }
  .val {
    white-space: nowrap;
  }
  .arrow {
    color: var(--muted);
    padding: 0 0.35rem;
  }
</style>
