<script lang="ts">
  // A Pokemon's Pokedex search and Unova Pokedex settings (core
  // regional_dex.rs and dex_search.rs, eidos tables regional_dex.txt and
  // pokedex.txt). A form has only its shape.
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { useParts } from "./parts.svelte";

  let {
    species,
    form = 0,
    nameOf,
    version,
    readOnly,
    onSaved,
  }: {
    species: number;
    /// 0: the species; 1 and up: one of its forms.
    form?: number;
    nameOf: (index: number) => string;
    version: number;
    readOnly: boolean;
    onSaved: () => void;
  } = $props();

  type Place = { kind: "unova" } | { kind: "number"; number: number } | { kind: "national" };
  interface Info {
    place: Place;
    game: Place;
    number: number | null;
    sharedWith: number | null;
  }
  interface Search {
    shape: number | null;
    letter: number | null;
  }
  interface SearchInfo {
    current: Search;
    game: Search;
    shapes: string[];
  }
  const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("");

  let info = $state<Info | null>(null);
  let search = $state<SearchInfo | null>(null);
  // Edited values.
  let inDex = $state(false);
  let ownNumber = $state(false);
  let number = $state(0);
  let shape = $state<number | null>(null);
  let letter = $state<number | null>(null);

  function reset() {
    const p = info?.place;
    inDex = !!p && p.kind !== "national";
    ownNumber = p?.kind === "number";
    number = p?.kind === "number" ? p.number : (info?.number ?? 0);
    shape = search?.current.shape ?? null;
    letter = search?.current.letter ?? null;
  }
  async function load() {
    const [i, s] = await Promise.all([
      form ? Promise.resolve(null) : invoke<Info>("regional_dex", { species }),
      invoke<SearchInfo>("dex_search", { species, form }),
    ]);
    info = i;
    search = s;
    untrack(reset);
  }
  $effect(() => {
    void version;
    void species;
    void form;
    load().catch(() => {
      info = null;
      search = null;
    });
  });

  const place = $derived<Place>(!inDex ? { kind: "national" } : ownNumber ? { kind: "number", number } : { kind: "unova" });
  const same = (a: Place, b: Place) => a.kind === b.kind && (a.kind !== "number" || (b.kind === "number" && a.number === b.number));
  const placeDirty = $derived(!!info && !same(place, info.place));
  const searchDirty = $derived(!!search && (shape !== search.current.shape || letter !== search.current.letter));
  const dirty = $derived(placeDirty || searchDirty);

  async function save() {
    if (placeDirty) await invoke("set_regional_dex", { species, place });
    if (searchDirty) await invoke("set_dex_search", { species, form, search: { shape, letter } });
    await load();
    onSaved();
  }
  const entryParts = useParts();
  $effect(() => {
    if (!entryParts || readOnly) return;
    return entryParts.register(`dex-${form}`, { dirty: () => dirty, save, discard: reset });
  });
  const optional = (v: string) => (v === "" ? null : Number(v));
</script>

{#if search}
  <div class="window block">
    <h3>Pokédex</h3>
    <div class="row">
      {#if info}
        <div class="field">
          <span class="name">Unova Pokédex</span>
          <div class="seg" role="radiogroup" aria-label="Unova Pokédex">
            <button type="button" role="radio" aria-checked={inDex} class:on={inDex} disabled={readOnly} onclick={() => (inDex = true)}>Listed</button>
            <button type="button" role="radio" aria-checked={!inDex} class:on={!inDex} disabled={readOnly} onclick={() => (inDex = false)}>National only</button>
          </div>
        </div>
        {#if inDex}
          <div class="field">
            <span class="name">Number</span>
            {#if ownNumber}
              <input type="number" min="0" max="998" value={number} disabled={readOnly} oninput={(e) => (number = Math.max(0, Math.min(998, Number(e.currentTarget.value) || 0)))} />
            {:else}
              <span class="auto">{placeDirty ? "Assigned on save" : (info.number ?? "")}</span>
            {/if}
            {#if !readOnly}
              <label class="check"><input type="checkbox" bind:checked={ownNumber} /> Custom number</label>
            {/if}
          </div>
        {/if}
      {/if}
      <label class="field">
        <span class="name">Shape</span>
        <select value={shape === null ? "" : String(shape)} disabled={readOnly} onchange={(e) => (shape = optional(e.currentTarget.value))}>
          <option value="">{form ? "Same as base form" : "None"}</option>
          {#each search.shapes as s, i}<option value={String(i)}>{s}</option>{/each}
        </select>
      </label>
      {#if !form}
        <label class="field">
          <span class="name">Letter</span>
          <select value={letter === null ? "" : String(letter)} disabled={readOnly} onchange={(e) => (letter = optional(e.currentTarget.value))}>
            <option value="">From name</option>
            {#each LETTERS as l, i}<option value={String(i)}>{l}</option>{/each}
          </select>
        </label>
      {/if}
    </div>
    {#if info?.sharedWith != null && inDex}
      <p class="muted note">{nameOf(info.sharedWith)} has the same number.</p>
    {/if}
  </div>
{/if}

<style>
  .block {
    padding: 0.9rem 1rem;
  }
  h3 {
    margin: 0;
  }
  .row {
    display: flex;
    align-items: flex-start;
    flex-wrap: wrap;
    gap: 0.8rem 1.2rem;
    margin-top: 0.6rem;
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
  .field input[type="number"] {
    width: 6rem;
  }
  .auto {
    min-height: 2rem;
    display: flex;
    align-items: center;
  }
  .check {
    font-size: 0.85rem;
    display: flex;
    gap: 0.35rem;
    align-items: center;
  }
  .note {
    font-size: 0.9rem;
    margin: 0.5rem 0 0;
  }
  .seg {
    display: inline-flex;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    overflow: hidden;
    background: var(--bg);
  }
  .seg button {
    border: none;
    border-radius: 0;
    box-shadow: none;
    background: none;
    padding: 0.3rem 0.6rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
  }
  .seg button + button {
    border-left: 1px solid var(--rim);
  }
  .seg button.on {
    background: var(--accent);
    color: var(--accent-ink);
  }
</style>
