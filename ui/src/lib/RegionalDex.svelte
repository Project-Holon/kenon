<script lang="ts">
  // A Pokemon's Unova Pokedex number, or national only (core
  // regional_dex.rs, eidos tables/regional_dex.txt). The game numbers it
  // from Victini 0 to Genesect 155.
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "morphe/menu.svelte";
  import { useParts } from "./parts.svelte";

  let {
    species,
    nameOf,
    version,
    readOnly,
    onSaved,
  }: {
    species: number;
    nameOf: (index: number) => string;
    version: number;
    readOnly: boolean;
    onSaved: () => void;
  } = $props();

  interface Info {
    place: number | null;
    game: number | null;
    next: number;
  }
  let info = $state<Info | null>(null);
  let inDex = $state(false);
  let number = $state(0);
  function reset() {
    inDex = info?.place != null;
    number = info?.place ?? info?.next ?? 0;
  }
  $effect(() => {
    void version;
    const s = species;
    invoke<Info>("regional_dex", { species: s })
      .then((i) => {
        info = i;
        untrack(reset);
      })
      .catch(() => (info = null));
  });
  const value = $derived(inDex ? number : null);
  const dirty = $derived(!!info && value !== info.place);

  async function save() {
    const other = await invoke<number | null>("set_regional_dex", { species, number: value });
    info = await invoke<Info>("regional_dex", { species });
    onSaved();
    if (other !== null) toast(`${nameOf(other)} also has Unova number ${value}`);
  }
  const entryParts = useParts();
  $effect(() => {
    if (!entryParts || readOnly) return;
    return entryParts.register("regional", { dirty: () => dirty, save, discard: reset });
  });
</script>

{#if info}
  <div class="window block">
    <h3>Unova Pokédex</h3>
    <div class="row">
      <div class="seg" role="radiogroup" aria-label="Unova Pokédex">
        <button type="button" role="radio" aria-checked={inDex} class:on={inDex} disabled={readOnly} onclick={() => (inDex = true)}>Listed</button>
        <button type="button" role="radio" aria-checked={!inDex} class:on={!inDex} disabled={readOnly} onclick={() => (inDex = false)}>National only</button>
      </div>
      {#if inDex}
        <label class="field">
          <span class="name">Number</span>
          <input type="number" min="0" max="998" value={number} disabled={readOnly} oninput={(e) => (number = Math.max(0, Math.min(998, Number(e.currentTarget.value) || 0)))} />
        </label>
      {/if}
    </div>
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
    align-items: flex-end;
    gap: 1rem;
    margin-top: 0.6rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    width: 7rem;
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
  .field .name {
    font-size: 0.85rem;
    color: var(--muted);
  }
</style>
