<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { asset } from "../lib/backend";
  import { once } from "../lib/cache.svelte";
  import { enumValues, saveRecord, tableRecord, type Section, type TableRecord, type Value } from "../lib/tables";
  import { play } from "../lib/sfx";

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
  // Species -> its icon's palette bank offset (the menu icons archive).
  let iconOffset = $state<Map<number, number>>(new Map());
  let error = $state<string | null>(null);

  async function load() {
    error = null;
    try {
      const r = await tableRecord("encounters", index, season);
      record = r;
      original = JSON.stringify(r);
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    load();
    enumValues("species").then((s) => (species = s));
    once("named/7", () => invoke<{ chr: number; paletteOffset: number }[] | null>("named_sheets", { arc: 7 }))
      .then((list) => (iconOffset = new Map((list ?? []).map((n) => [n.chr, n.paletteOffset]))))
      .catch(() => {});
  });

  const rates = $derived((record?.sections.find((s) => s.kind === "fields") as { values: Value[] } | undefined)?.values ?? []);
  const lists = $derived((record?.sections.filter((s) => s.kind === "list") ?? []) as List[]);
  const rateOf = (i: number) => rates.find((v) => v.key.startsWith("rate") && v.key.endsWith(String(i + 1))) ?? rates[i];
  const used = (l: List) => l.rows.some((r) => r[0] > 0);
  const dirty = $derived(record !== null && JSON.stringify(record) !== original);
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

  function icon(sp: number): string | null {
    const chr = 7 + 2 * sp;
    if (sp <= 0 || !iconOffset.has(chr)) return null;
    return asset(`preview/7:${chr}/7:0?cells=2&paloffset=${iconOffset.get(chr)}`);
  }
</script>

<div class="wild">
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
          <span class="muted">How often</span>
          <input type="number" min={r.min} max={r.max} bind:value={r.value} readonly={readOnly} />
          {#if r.value === 0}<span class="muted">off</span>{/if}
        </label>
      {/if}
      <ul class="slots">
        {#each l.rows as row, i}
          <li>
            <span class="pct">{(l.rows.length === 12 ? LAND : WATER)[i] ?? ""}%</span>
            {#if icon(row[sp])}<img class="px icon" src={icon(row[sp])} alt="" />{:else}<span class="icon"></span>{/if}
            <select bind:value={row[sp]} disabled={readOnly} aria-label="Species">
              {#each species as n, k}<option value={k}>{n || "None"}</option>{/each}
            </select>
            <span class="lv">Lv</span>
            <input class="num" type="number" min="0" max="100" bind:value={row[min]} readonly={readOnly} aria-label="Lowest level" />
            <span class="muted">-</span>
            <input class="num" type="number" min="0" max="100" bind:value={row[max]} readonly={readOnly} aria-label="Highest level" />
            {#if form >= 0 && row[form] !== 0}
              <input class="num" type="number" min="0" max="31" bind:value={row[form]} readonly={readOnly} title="Form" aria-label="Form" />
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
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
  .slots li {
    display: grid;
    grid-template-columns: 2.4rem 32px minmax(0, 1fr) auto 3rem auto 3rem auto;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
  }
  .slots select {
    min-width: 0;
    width: 100%;
  }
  .pct {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .icon {
    width: 32px;
    height: 32px;
    image-rendering: pixelated;
    object-fit: contain;
  }
  .num {
    width: 100%;
    min-width: 0;
  }
  .lv {
    color: var(--muted);
    font-size: 0.75rem;
  }
</style>
