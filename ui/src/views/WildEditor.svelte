<script lang="ts">
  import { onMount } from "svelte";
  import { enumValues, saveRecord, tableRecord, type Section, type TableRecord, type Value } from "../lib/tables";
  import { play } from "../lib/sfx";
  import SearchSelect from "../lib/SearchSelect.svelte";
  import { loadIcons, speciesIcon } from "../lib/icons.svelte";

  // A map's wild Pokemon, one method at a time: each slot on one line.
  let { index, readOnly, onSaved }: { index: number; readOnly: boolean; onSaved: () => void } = $props();

  type List = Extract<Section, { kind: "list" }>;

  const SEASONS = ["Spring", "Summer", "Autumn", "Winter"];
  // Chance of each slot, from the game's slot tables (12 land, 5 water).
  const LAND = [20, 20, 10, 10, 10, 10, 5, 5, 4, 4, 1, 1];
  const WATER = [60, 30, 5, 4, 1];

  let record = $state<TableRecord | null>(null);
  let original = $state("");
  let season = $state(0);
  let method = $state(0);
  let species = $state<string[]>([]);
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function load() {
    error = null;
    try {
      const r = await tableRecord("encounters", index, season);
      record = r;
      original = JSON.stringify(r);
      if (method >= r.sections.filter((x) => x.kind === "list").length) method = 0;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    loadIcons();
    load();
    enumValues("species").then((s) => (species = s));
  });

  // The species list is long: build the options once, not per slot.
  const speciesOptions = $derived(species.map((n, k) => [k, n || "None"] as [number, string]));
  const rates = $derived((record?.sections.find((s) => s.kind === "fields") as { values: Value[] } | undefined)?.values ?? []);
  const lists = $derived((record?.sections.filter((s) => s.kind === "list") ?? []) as List[]);
  const rateOf = (i: number) => rates.find((v) => v.key.startsWith("rate") && v.key.endsWith(String(i + 1))) ?? rates[i];
  const used = (l: List) => l.rows.some((r) => r[0] > 0);
  const dirty = $derived(record !== null && JSON.stringify(record) !== original);
  // Slot odds where they are known: land and surfing slots.
  const odds = (l: List, i: number) => {
    const t = l.rows.length === 12 ? LAND : l.key.startsWith("surf") ? WATER : null;
    return t ? `${t[i]}%` : "";
  };
  const NAMES: Record<string, string> = {
    grass: "Grass",
    doubleGrass: "Dark grass",
    shakingGrass: "Shaking grass",
    surf: "Surfing",
    surfRipple: "Surf ripples",
    fishing: "Fishing",
    fishingRipple: "Fishing ripples",
  };
  const label = (l: List) => NAMES[l.key] ?? l.label.replace(/ slots$/, "");
  const col = (l: List, name: string) => l.columns.findIndex((c) => c.name === name || c.label.toLowerCase().startsWith(name));

  // Levels stay between 0 and 100, and the highest never sits under the lowest.
  function levels(row: number[], lo: number, hi: number, edited: "lo" | "hi") {
    for (const k of [lo, hi]) {
      const n = Math.round(Number(row[k]));
      row[k] = Math.min(100, Math.max(0, Number.isFinite(n) ? n : 0));
    }
    if (row[hi] < row[lo]) row[edited === "lo" ? hi : lo] = row[edited === "lo" ? lo : hi];
  }
  function fixForm(row: number[], at: number) {
    const n = Math.round(Number(row[at]));
    row[at] = Math.min(31, Math.max(0, Number.isFinite(n) ? n : 0));
  }

  function keys(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      if (!readOnly && dirty && !busy) save();
    }
  }

  async function save() {
    if (!record) return;
    busy = true;
    try {
      const values: Record<string, number> = {};
      const out: Record<string, number[][]> = {};
      for (const s of record.sections) {
        if (s.kind === "fields") s.values.forEach((v) => (values[v.key] = Number(v.value)));
        else out[s.key] = s.rows.map((r) => r.map(Number));
      }
      await saveRecord("encounters", index, { values, lists: out, season });
      play("save");
      onSaved();
      await load();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }
</script>

<div class="wild" role="presentation" onkeydown={keys}>
  <div class="top">
    {#if (record?.seasons ?? 1) > 1}
      <select bind:value={season} onchange={load} aria-label="Season">
        {#each SEASONS.slice(0, record?.seasons ?? 1) as s, i}<option value={i}>{s}</option>{/each}
      </select>
    {/if}
    <div class="spacer"></div>
    {#if !readOnly}
      <button disabled={!dirty || busy} onclick={load}>Discard</button>
      <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
    {/if}
  </div>
  {#if error}<p class="bad">{error}</p>{/if}

  {#if record}
    <div class="methods" role="tablist">
      {#each lists as l, i}
        <button role="tab" aria-selected={method === i} class:primary={method === i} class:empty={!used(l)} onclick={() => (method = i)}>{label(l)}</button>
      {/each}
    </div>

    {#if lists[method]}
      {@const l = lists[method]}
      {@const r = rateOf(method)}
      {@const [sp, form, min, max] = [col(l, "species"), col(l, "form"), col(l, "min"), col(l, "max")]}
      {#if r}
        <label class="rate">
          <span class="muted">Encounter rate</span>
          <input type="number" min={r.min} max={r.max} bind:value={r.value} readonly={readOnly} />
          {#if r.value === 0}<span class="muted">off</span>{/if}
        </label>
      {/if}
      <div class="slots-head muted">
        <span>Odds</span>
        <span>Pokemon</span>
        <span>Level</span>
        {#if form >= 0}<span>Form</span>{/if}
      </div>
      <ul class="slots">
        {#each l.rows as row, i}
          <li>
            <span class="pct">{odds(l, i)}</span>
            <SearchSelect bind:value={row[sp]} options={speciesOptions} icon={speciesIcon} disabled={readOnly} label="Pokemon in slot {i + 1}" />
            <span class="levels">
              <input class="num" type="number" min="0" max="100" bind:value={row[min]} onchange={() => levels(row, min, max, "lo")} readonly={readOnly} aria-label="Lowest level" />
              <span class="muted">-</span>
              <input class="num" type="number" min="0" max="100" bind:value={row[max]} onchange={() => levels(row, min, max, "hi")} readonly={readOnly} aria-label="Highest level" />
            </span>
            {#if form >= 0}
              <input class="num" type="number" min="0" max="31" bind:value={row[form]} onchange={() => fixForm(row, form)} readonly={readOnly} aria-label="Form" />
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {:else if !error}
    <p class="muted">Loading...</p>
  {/if}
</div>

<style>
  .wild {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .top button {
    padding: 0.2rem 0.6rem;
  }
  .spacer {
    flex: 1;
  }
  .methods {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }
  .methods button {
    padding: 0.15rem 0.5rem;
    font-size: 0.8rem;
  }
  .methods .empty:not(.primary) {
    opacity: 0.5;
  }
  .rate {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
  }
  .rate input {
    width: 4rem;
  }
  .slots {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .slots li,
  .slots-head {
    display: grid;
    grid-template-columns: 2.4rem minmax(0, 1fr) 7.5rem;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
  }
  .slots-head {
    font-size: 0.75rem;
    padding-bottom: 0.1rem;
  }
  .slots-head:has(span:nth-child(4)),
  .slots:has(input[aria-label="Form"]) li {
    grid-template-columns: 2.4rem minmax(0, 1fr) 7.5rem 3rem;
  }
  .levels {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }
  .pct {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .num {
    width: 100%;
    min-width: 0;
  }
</style>
