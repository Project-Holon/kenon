<script lang="ts">
  // "In battle": which battle code a move, ability or item runs and the
  // numbers in it (core/src/handlers.rs, eidos tables/<kind>_handlers.txt).
  // It can work like several entries at once, in order; the keys each base
  // takes and their built-in values come from eidos's handler_params.json,
  // and only those are offered.
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { enumValues } from "./tables";
  import { play } from "./sfx";
  import { toast } from "morphe/menu.svelte";
  import { useParts } from "./parts.svelte";

  let {
    table,
    id,
    last,
    version,
    readOnly,
    onSaved,
  }: {
    table: "moves" | "items" | "abilities";
    id: number;
    /// The last built-in id; higher ids are added by the mod.
    last: number;
    version: number;
    readOnly: boolean;
    onSaved: () => void;
  } = $props();

  type Builtin = string | number;
  interface Entry {
    keys: Record<string, Builtin>;
    notes: string[];
    work?: number[];
  }
  interface Params {
    source: string;
    eidos: string | null;
    keys: Record<string, (string | number)[]>;
    entries: Record<string, Entry> | null;
  }
  interface Part {
    base: number;
    keys: Record<string, string>;
  }
  /// One base being edited: every key it takes, with the value shown.
  interface Edit {
    base: number;
    values: Record<string, string>;
  }

  const added = $derived(id > last);

  let params = $state<Params | null>(null);
  let rows = $state<Record<string, Part[]>>({});
  let names = $state<string[]>([]);
  let error = $state("");
  $effect(() => {
    const t = table;
    invoke<Params>("handler_params", { table: t })
      .then((p) => (params = p))
      .catch((e) => (error = String(e)));
    enumValues(t === "moves" ? "move" : t === "items" ? "item" : "ability").then((n) => (names = n));
  });
  $effect(() => {
    void version;
    const t = table;
    invoke<Record<string, Part[]>>("handler_rows", { table: t })
      .then((r) => (rows = r))
      .catch(() => (rows = {}));
  });

  const entryOf = (b: number) => params?.entries?.[String(b)] ?? null;
  // Several bases at once needs an eidos that knows which ones clash: its
  // list then names each base's work slots.
  const multi = $derived(!!params?.entries && Object.values(params.entries).some((e) => "work" in e));

  let parts = $state<Edit[]>([]);
  let saving = $state(false);
  const saved = $derived<Part[]>(rows[String(id)] ?? (added ? [] : [{ base: id, keys: {} }]));

  function editOf(p: Part): Edit {
    const values: Record<string, string> = {};
    for (const [k, b] of Object.entries(entryOf(p.base)?.keys ?? {})) values[k] = p.keys[k] ?? String(b);
    return { base: p.base, values };
  }
  function reset() {
    parts = saved.map(editOf);
  }
  // Reloads the saved setting when the entry, the rows or the list change;
  // untracked, so an edit (which reads and writes parts) does not undo itself.
  $effect(() => {
    void id;
    void saved;
    void params;
    untrack(reset);
  });

  const isSame = (b: Builtin | undefined, v: string) => b !== undefined && (String(b) === v || (typeof b === "number" && Number(v) === b));
  function changed(e: Edit): Part {
    const builtin = entryOf(e.base)?.keys ?? {};
    return { base: e.base, keys: Object.fromEntries(Object.entries(e.values).filter(([k, v]) => !isSame(builtin[k], v))) };
  }
  const current = $derived(parts.map(changed));
  const dirty = $derived.by(() => {
    if (current.length !== saved.length) return true;
    return current.some((p, i) => {
      const s = saved[i];
      if (p.base !== s.base) return true;
      const keys = new Set([...Object.keys(p.keys), ...Object.keys(s.keys)]);
      return [...keys].some((k) => p.keys[k] === undefined || s.keys[k] === undefined || !(p.keys[k] === s.keys[k] || Number(p.keys[k]) === Number(s.keys[k])));
    });
  });

  function setBase(i: number, b: number | null) {
    if (b === null) parts = parts.filter((_, j) => j !== i);
    else parts = parts.map((p, j) => (j === i ? editOf({ base: b, keys: {} }) : p));
  }
  function setValue(i: number, k: string, v: string) {
    parts = parts.map((p, j) => (j === i ? { ...p, values: { ...p.values, [k]: v } } : p));
  }
  function addPart() {
    const free = bases.find((b) => !blocked(b.i, parts.length));
    if (free) parts = [...parts, editOf({ base: free.i, keys: {} })];
  }

  async function save(quiet = false) {
    saving = true;
    try {
      await invoke("set_handler", { table, id, row: current.length ? current : null });
      rows = await invoke<Record<string, Part[]>>("handler_rows", { table });
      onSaved();
      if (!quiet) {
        play("save");
        toast(`Saved what ${names[id] ?? "it"} does in battle`);
      }
    } catch (e) {
      if (quiet) throw e;
      toast(String(e));
    } finally {
      saving = false;
    }
  }

  // Inside an entry screen, the screen's Save and Discard cover this panel.
  const entryParts = useParts();
  $effect(() => {
    if (!entryParts || readOnly) return;
    return entryParts.register("battle", {
      dirty: () => dirty,
      save: () => save(true),
      discard: () => reset(),
    });
  });

  // Plain words for keys and values (eidos sdk/battle_handlers.h).
  const KEY_LABEL: Record<string, string> = {
    stat: "Stat",
    stages: "Stages",
    status: "Condition",
    weather: "Weather",
    type: "Type",
    chance: "Chance (%)",
    ratio: "Multiplier",
    hp: "HP amount",
  };
  const STAT_LABEL: Record<string, string> = {
    atk: "Attack",
    def: "Defense",
    spatk: "Sp. Atk",
    spdef: "Sp. Def",
    speed: "Speed",
    accuracy: "Accuracy",
    evasion: "Evasion",
  };
  const WEATHER_LABEL: Record<string, string> = { sun: "Harsh sunlight", rain: "Rain", hail: "Hail", sand: "Sandstorm" };
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  function keyLabel(e: Entry, k: string) {
    const [name, n] = k.split(".");
    const l = KEY_LABEL[name] ?? cap(name);
    return n ? `${l} (place ${n})` : `${name}.2` in e.keys ? `${l} (place 1)` : l;
  }
  function choices(k: string): [string, string][] {
    const name = k.split(".")[0];
    const allowed = params?.keys[name] ?? [];
    if (name === "stat") return allowed.map((s) => [String(s), STAT_LABEL[String(s)] ?? String(s)]);
    if (name === "weather") return allowed.map((s) => [String(s), WEATHER_LABEL[String(s)] ?? cap(String(s))]);
    if (name === "status" || name === "type") return allowed.map((s) => [String(s), cap(String(s))]);
    if (name === "stages") {
      const out: [string, string][] = [];
      for (let n = Number(allowed[0] ?? -6); n <= Number(allowed[1] ?? 6); n++) if (n) out.push([String(n), n > 0 ? `+${n}` : String(n)]);
      return out;
    }
    if (name === "hp") {
      const out: [string, string][] = [];
      for (let d = 1; d <= 16; d++) out.push([`1/${d}`, d === 1 ? "All of max HP" : `1/${d} of max HP`]);
      return out;
    }
    return [];
  }
  const numeric = (k: string) => ["chance", "ratio"].includes(k.split(".")[0]);
  const range = (k: string) => (params?.keys[k.split(".")[0]] ?? [0, 100]).map(Number);
  const builtinLabel = (e: Entry, k: string) => choices(k).find(([v]) => v === String(e.keys[k]))?.[1] ?? String(e.keys[k]);

  // The base list: built-in entries by name, those with numbers marked.
  const bases = $derived(
    names
      .map((name, i) => ({ i, name }))
      .filter((e) => e.i > 0 && e.i <= last && e.name)
      .sort((a, b) => a.name.localeCompare(b.name)),
  );
  const adjustable = (i: number) => Object.keys(entryOf(i)?.keys ?? {}).length > 0;
  /// Whether base b cannot go in place `at`: already picked elsewhere, or it
  /// shares a work slot with another pick.
  function blocked(b: number, at: number) {
    // A base whose own line joins several bases cannot be joined again
    // (eidos refuses it).
    if (b !== id && (rows[String(b)]?.length ?? 0) > 1) return true;
    const slots = entryOf(b)?.work ?? [];
    return parts.some((p, j) => j !== at && (p.base === b || (entryOf(p.base)?.work ?? []).some((s) => slots.includes(s))));
  }
  // Number boxes in reading order: what changes, then by how much and how
  // often.
  const KEY_ORDER = ["stat", "stages", "status", "type", "weather", "chance", "ratio", "hp"];
  const keyRank = (k: string) => {
    const [name, n] = k.split(".");
    const r = KEY_ORDER.indexOf(name);
    return (r < 0 ? KEY_ORDER.length : r) * 10 + Number(n ?? 1);
  };
  const orderedKeys = (e: Entry) => Object.keys(e.keys).sort((a, b) => keyRank(a) - keyRank(b));
  /// eidos takes up to four bases on one line.
  const MAX_BASES = 4;
  const nameOf = (b: number) => (b === id && !added ? `Itself (${names[id] ?? id})` : (names[b] ?? String(b)));
</script>

<div class="window block handler">
  <div class="top">
    <h3>In battle</h3>
    {#if !readOnly && params && !entryParts}
      <div class="acts">
        {#if dirty}<button class="quiet" onclick={reset} disabled={saving}>Discard</button>{/if}
        <button class="primary" onclick={() => save()} disabled={!dirty || saving}>{saving ? "Saving..." : "Save"}</button>
      </div>
    {/if}
  </div>
  {#if error}
    <p class="bad">{error}</p>
  {:else if !params}
    <p class="muted">Loading...</p>
  {:else}
    {#if !parts.length}
      <label class="field works">
        <span class="name">Works like</span>
        <select value="" disabled={readOnly} onchange={(e) => e.currentTarget.value !== "" && (parts = [editOf({ base: Number(e.currentTarget.value), keys: {} })])}>
          <option value="">None (no battle effect)</option>
          {#each bases as b}<option value={String(b.i)}>{b.name}{adjustable(b.i) ? " (numbers)" : ""}</option>{/each}
        </select>
      </label>
    {/if}
    {#each parts as p, i (i)}
      {@const e = entryOf(p.base)}
      <div class="part" class:more={i > 0}>
        <div class="works-row">
          <label class="field works">
            <span class="name">{i ? "And works like" : "Works like"}</span>
            <select value={String(p.base)} disabled={readOnly} onchange={(ev) => setBase(i, Number(ev.currentTarget.value))}>
              {#if !added && i === 0}<option value={String(id)}>{nameOf(id)}</option>{/if}
              {#each bases.filter((b) => b.i !== id || added) as b}
                <option value={String(b.i)} disabled={blocked(b.i, i)}>{b.name}{adjustable(b.i) ? " (numbers)" : ""}</option>
              {/each}
            </select>
          </label>
          {#if !readOnly && (parts.length > 1 || added)}
            <button class="quiet" onclick={() => setBase(i, null)} title="Remove this effect">Remove</button>
          {/if}
        </div>
        {#if e && Object.keys(e.keys).length}
          <div class="grid">
            {#each orderedKeys(e) as k}
              <label class="field">
                <span class="name">{keyLabel(e, k)}</span>
                {#if numeric(k)}
                  <input
                    type="number"
                    min={range(k)[0]}
                    max={range(k)[1]}
                    step={k.startsWith("ratio") ? 0.05 : 1}
                    value={p.values[k]}
                    disabled={readOnly}
                    oninput={(ev) => setValue(i, k, ev.currentTarget.value)}
                  />
                {:else}
                  <select value={p.values[k]} disabled={readOnly} onchange={(ev) => setValue(i, k, ev.currentTarget.value)}>
                    {#each choices(k) as [v, l]}<option value={v}>{l}</option>{/each}
                  </select>
                {/if}
                {#if !isSame(e.keys[k], p.values[k])}
                  <span class="muted small">Built in: {builtinLabel(e, k)}</span>
                {/if}
              </label>
            {/each}
          </div>
        {:else}
          <p class="muted note">{p.base === id ? "Its" : `${names[p.base] ?? p.base}'s`} handler has no editable parameters.</p>
        {/if}
        {#if e?.notes.length}
          <p class="muted note">
            {e.notes.length === 1 ? "One additional parameter is" : `${e.notes.length} additional parameters are`} computed at runtime
            and cannot be edited.
          </p>
        {/if}
      </div>
    {/each}
    {#if multi && parts.length && parts.length < MAX_BASES && !readOnly}
      <button class="quiet add" onclick={addPart}>Add handler...</button>
    {/if}
    {#if parts.some((p) => p.base !== id)}
      <p class="muted note">
        Checks for the source {table === "abilities" ? "ability" : table === "moves" ? "move" : "item"}'s ID
        elsewhere in the game are not included.{#if parts.length > 1} Overlapping effects all run, in this order.{/if}
      </p>
    {/if}
  {/if}
</div>

<style>
  .block {
    padding: 0.9rem 1rem;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .top h3 {
    flex: 1;
    margin: 0;
  }
  .acts {
    display: flex;
    gap: 0.5rem;
  }
  .part.more {
    margin-top: 0.9rem;
    padding-top: 0.8rem;
    border-top: 1px solid var(--rim);
  }
  .works-row {
    display: flex;
    align-items: flex-end;
    gap: 0.75rem;
    margin-top: 0.6rem;
  }
  .works {
    width: 24rem;
    max-width: 100%;
  }
  .add {
    margin-top: 0.8rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
    gap: 0.6rem 1rem;
    margin-top: 0.7rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .field .name {
    font-size: 0.85rem;
    color: var(--muted);
  }
  .note {
    font-size: 0.9rem;
    margin: 0.5rem 0 0;
  }
  .small {
    font-size: 0.8rem;
  }
  .bad {
    color: var(--bad);
  }
</style>
