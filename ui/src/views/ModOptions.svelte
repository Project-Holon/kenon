<script lang="ts">
  import { onMount } from "svelte";
  import { debounced } from "morphe/cache.svelte";
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
  const TEXT_MAX = 64;
  const kinds = [
    { id: "bool", label: "On or off" },
    { id: "choice", label: "Choice" },
    { id: "int", label: "Number" },
    { id: "text", label: "Text" },
    { id: "secret", label: "Password" },
  ] as const;
  const entryNames: Record<OptionDecl["entry"], string> = {
    option: "Option",
    tab: "Tab",
    heading: "Heading",
    help: "Help",
    button: "Button",
    show: "Shown text",
  };

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
        when: o.when.trim(),
        tags: o.tags.trim(),
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
  const optionCount = $derived(options.filter((o) => o.entry === "option").length);

  const blank = (entry: OptionDecl["entry"], key = "", label = ""): OptionDecl => ({
    entry,
    key,
    label,
    kind: "bool",
    default: 1,
    min: 0,
    max: 10,
    values: [],
    text: "",
    word: false,
    when: "",
    tags: "",
    line: 0,
    lead: [],
    comment: "",
    raw: "",
  });

  function freeKey(base: string) {
    let n = 1;
    while (options.some((o) => o.key === `${base}${n}`)) n++;
    return `${base}${n}`;
  }

  function add(entry: OptionDecl["entry"]) {
    if (entry === "option") options.push(blank("option", freeKey("option")));
    else if (entry === "button") options.push(blank("button", freeKey("button")));
    else if (entry === "show") options.push(blank("show", freeKey("shown")));
    else options.push(blank(entry));
  }

  /// A help line right after the option it explains.
  function addHelp(i: number) {
    options.splice(i + 1, 0, blank("help", options[i].key));
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
    if (kind !== "text") o.word = false;
    if (kind === "bool") o.default = o.default ? 1 : 0;
    if (kind === "choice") {
      o.default = 0;
      if (!o.values.length) o.values = ["off", "on"];
    }
    if (kind === "int") {
      if (!(o.min <= o.max)) o.max = o.min;
      o.default = Math.min(Math.max(o.default, o.min), o.max);
    }
    if (kind === "text" || kind === "secret") {
      if (!(o.max >= 1 && o.max <= TEXT_MAX)) o.max = 20;
      if (kind === "secret") o.text = "";
      // a lobby rule is a bool, choice or int
      o.tags = o.tags
        .split(",")
        .filter((t) => t.trim() !== "lobby")
        .join(",");
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
    if (o.kind === "text") return o.text || "...";
    if (o.kind === "secret") return "••••";
    return String(Number.isFinite(o.default) ? o.default : 0);
  };

  // The options above a help line, for its choice of key.
  const optionsAbove = (i: number) => options.slice(0, i).filter((o) => o.entry === "option" && o.key.trim());

  // The preview: the mod's row, then each tab with its rows.
  const groups = $derived.by(() => {
    const out: { title: string; rows: OptionDecl[] }[] = [{ title: modName || "Mod", rows: [] }];
    for (const o of options) {
      if (o.entry === "tab") out.push({ title: o.label.trim() || "(no title)", rows: [] });
      else if (o.entry !== "help") out[out.length - 1].rows.push(o);
    }
    return out.filter((g, i) => i > 0 || g.rows.length);
  });
</script>

<div class="options">
  <div class="bar">
    <span class="muted">{path ?? "A new file, options.txt, is made on save."}</span>
    <div class="spacer"></div>
    <button class="quiet" disabled={optionCount >= LIMIT} onclick={() => add("option")}>Add option</button>
    <button class="quiet" onclick={() => add("tab")} title="A Settings tab of its own for the rows below it">Tab</button>
    <button class="quiet" onclick={() => add("heading")}>Heading</button>
    <button class="quiet" onclick={() => add("button")} title="A row that sends setting.button to the mod's code">Button</button>
    <button class="quiet" onclick={() => add("show")} title="A row of text the mod's code sets; hidden while empty">Shown text</button>
    <button class="quiet" disabled={!dirty || busy} onclick={() => take({ path, exists: true, options: JSON.parse(original), diagnostics: [] })}>Undo</button>
    <button class="primary" disabled={!dirty || busy || blocked} onclick={save}>Save</button>
  </div>

  <div class="scroll">
    {#if error}<p class="bad">{error}</p>{/if}
    {#each ignored as p}<p class="bad">Line {p.line}: {p.message}</p>{/each}

    {#if !options.length}
      <p class="muted">
        No options. Options show on the game's settings screen under the mod's name, or on a tab of their own, and rules
        read them as setting.key.
      </p>
    {/if}

    <ol class="list">
      {#each options as o, i}
        <li class="item" class:minor={o.entry === "help" || o.entry === "heading"} class:tab={o.entry === "tab"}>
          <div class="head">
            <span class="move">
              <button class="step" disabled={i === 0} onclick={() => move(i, -1)} aria-label="Move up">&#9650;</button>
              <button class="step" disabled={i === options.length - 1} onclick={() => move(i, 1)} aria-label="Move down">&#9660;</button>
            </span>
            {#if o.entry === "option"}
              <select value={o.kind} aria-label="Kind" onchange={(e) => setKind(o, e.currentTarget.value as OptionDecl["kind"])}>
                {#each kinds as k}<option value={k.id}>{k.label}</option>{/each}
              </select>
            {:else}
              <span class="badge">{entryNames[o.entry]}</span>
            {/if}
            {#if o.entry === "option" || o.entry === "button" || o.entry === "show"}
              <input class="key" aria-label="Key" placeholder="key" maxlength="63" bind:value={o.key} />
              <input class="label" aria-label="Label" placeholder="Label" maxlength="63" bind:value={o.label} />
            {:else if o.entry === "help"}
              <select class="key" aria-label="Option it explains" bind:value={o.key}>
                {#each optionsAbove(i) as a}<option value={a.key}>{a.key}</option>{/each}
                {#if !optionsAbove(i).some((a) => a.key === o.key)}<option value={o.key}>{o.key || "(choose)"}</option>{/if}
              </select>
              <input class="label" aria-label="Help text" placeholder="What the option does" bind:value={o.label} />
            {:else}
              <input
                class="label"
                aria-label={o.entry === "tab" ? "Tab title" : "Heading"}
                placeholder={o.entry === "tab" ? "Tab title" : "Heading"}
                maxlength={o.entry === "tab" ? 31 : 63}
                bind:value={o.label}
              />
            {/if}
            {#if o.entry === "option"}
              <button class="quiet" onclick={() => addHelp(i)} title="A line of help shown beside the option">Help</button>
            {/if}
            <button class="quiet" onclick={() => remove(i)} aria-label="Remove">Remove</button>
          </div>

          {#if o.entry === "option"}
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
              {:else if o.kind === "int"}
                <div class="ints">
                  <label>Lowest <input type="number" step="1" bind:value={o.min} /></label>
                  <label>Highest <input type="number" step="1" bind:value={o.max} /></label>
                  <label>Default <input type="number" step="1" bind:value={o.default} /></label>
                </div>
              {:else}
                <div class="ints">
                  <label>Longest <input type="number" min="1" max={TEXT_MAX} step="1" bind:value={o.max} /></label>
                  {#if o.kind === "text"}
                    <label>Default <input maxlength={TEXT_MAX} bind:value={o.text} /></label>
                    <label class="inline" title="The game's keyboard has no space key">
                      <input type="checkbox" bind:checked={o.word} /> One word
                    </label>
                  {:else}
                    <span class="muted small">Kept in memory only, never saved.</span>
                  {/if}
                </div>
              {/if}
            </div>
          {/if}
          {#if o.entry === "option" || o.entry === "button" || o.entry === "show"}
            <div class="body ends">
              <label>Show when <input placeholder="key=value or key!=value" bind:value={o.when} /></label>
              <label>Only in <input placeholder="lobby, or mode names" bind:value={o.tags} /></label>
            </div>
          {/if}
          {#each about(i) as p}<p class={p.severity === "error" ? "bad" : "warn"}>{p.message}</p>{/each}
        </li>
      {/each}
    </ol>

    {#if groups.length}
      <h3>Settings screen</h3>
      {#each groups as g}
        <div class="preview" aria-label="Preview">
          <p class="mod">{g.title}</p>
          {#each g.rows as o}
            {#if o.entry === "heading"}
              <p class="phead">{o.label.trim() || "(heading)"}</p>
            {:else}
              <div class="prow">
                <span>
                  {o.label.trim() || "(no label)"}
                  {#if o.when.trim()}<span class="muted small">when {o.when.trim()}</span>{/if}
                </span>
                <span class="val">
                  {#if o.entry === "button"}
                    <span class="muted">button</span>
                  {:else if o.entry === "show"}
                    <span class="muted">set by the mod</span>
                  {:else}
                    {@const steps = o.kind === "choice" || o.kind === "int"}
                    {#if steps}<span class="arrow">&lsaquo;</span>{/if}{shown(o)}{#if steps}<span class="arrow">&rsaquo;</span>{/if}
                  {/if}
                </span>
              </div>
            {/if}
          {/each}
        </div>
      {/each}
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
    flex-wrap: wrap;
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
  .item.minor {
    background: var(--panel-2);
    margin-left: 2rem;
  }
  .item.tab {
    border-color: var(--accent, var(--gold));
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
  .badge {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
    min-width: 5.5rem;
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
  .ends {
    display: flex;
    flex-wrap: wrap;
    gap: 0.7rem;
    font-size: 0.9rem;
  }
  .ends label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .ends input {
    width: 12rem;
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
    align-items: center;
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
  .small {
    font-size: 0.8rem;
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
    margin-bottom: 0.6rem;
  }
  .mod {
    margin: 0;
    font-weight: 700;
    color: var(--muted);
  }
  .phead {
    margin: 0.3rem 0 0;
    font-weight: 600;
    font-size: 0.9rem;
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
  .prow .small {
    margin-left: 0.4rem;
  }
  .val {
    white-space: nowrap;
  }
  .arrow {
    color: var(--muted);
    padding: 0 0.35rem;
  }
</style>
