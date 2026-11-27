<script lang="ts">
  // A Pokemon's forms and cry (core species_add.rs, cries.rs). Forms are
  // only for the Pokemon the mod adds; any Pokemon or form can change its
  // cry: another Pokemon's, or a brand-new one from a WAV.
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { asset } from "./backend";
  import { play } from "./sfx";
  import { toast } from "morphe/menu.svelte";

  let {
    id,
    speciesNew,
    forms,
    nameOf,
    version,
    readOnly,
    onChanged,
    onOpen,
  }: {
    /// The entry shown: a species or a form slot.
    id: number;
    speciesNew: number[];
    /// Species to its form slots, form 1 first.
    forms: Record<string, number[]>;
    nameOf: (index: number) => string;
    version: number;
    readOnly: boolean;
    /// After a change to the slots: reload the list, then open `index`.
    onChanged: (index: number) => Promise<void>;
    onOpen: (index: number) => void;
  } = $props();

  /// The species and form this entry is.
  const owner = $derived.by((): [number, number] => {
    for (const [s, slots] of Object.entries(forms)) {
      const f = slots.indexOf(id);
      if (f >= 0) return [Number(s), f + 1];
    }
    return [id, 0];
  });
  const species = $derived(owner[0]);
  const form = $derived(owner[1]);
  const isNew = $derived(speciesNew.includes(species));
  const formSlots = $derived(forms[String(species)] ?? []);

  let busy = $state(false);
  async function run(what: () => Promise<void>) {
    busy = true;
    try {
      await what();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  // Forms.
  let source = $state(0);
  $effect(() => {
    source = species;
  });
  const sources = $derived.by(() => {
    const out: { i: number; name: string }[] = [];
    for (let i = 1; i <= 649; i++) out.push({ i, name: nameOf(i) });
    for (const s of speciesNew) {
      out.push({ i: s, name: nameOf(s) });
      for (const f of forms[String(s)] ?? []) out.push({ i: f, name: nameOf(f) });
    }
    return out.sort((a, b) => a.name.localeCompare(b.name));
  });
  function addForm() {
    run(async () => {
      const f = await invoke<number>("species_add_form", { species, from: source });
      play("save");
      toast(`Added form ${f} to ${nameOf(species)}`);
      const slots = await invoke<Record<string, number[]>>("species_forms");
      await onChanged(slots[String(species)]?.[f - 1] ?? species);
    });
  }
  let confirmRemove = $state(false);
  $effect(() => {
    void id;
    confirmRemove = false;
  });
  function removeForm() {
    if (!confirmRemove) {
      confirmRemove = true;
      return;
    }
    run(async () => {
      const r = await invoke<{ updated: string[] }>("species_remove_form", { species, form });
      play("save");
      const n = r.updated.length;
      toast(`Removed form ${form} from ${nameOf(species)}${n ? `. ${n} record${n > 1 ? "s" : ""} now use the base form.` : ""}`);
      await onChanged(species);
    });
  }

  // Cry.
  interface Cry {
    slot: number;
    default: number;
    name: string;
  }
  let cry = $state<Cry | null>(null);
  $effect(() => {
    void version;
    const [s, f] = owner;
    invoke<Cry>("species_cry", { species: s, form: f })
      .then((c) => (cry = c))
      .catch(() => (cry = null));
  });
  async function reloadCry() {
    cry = await invoke<Cry>("species_cry", { species, form });
  }
  /// "Pikachu", "Shaymin (Sky Forme)" or the new cry's name.
  function cryLabel(c: Cry) {
    if (c.slot >= 1 && c.slot <= 649) return `${nameOf(c.slot)}'s cry`;
    if (c.slot === 650) return "Shaymin (Sky Forme)'s cry";
    const n = c.name.replace(/^WAVE_CRY_/, "");
    return n ? `Custom cry: ${n.charAt(0) + n.slice(1).toLowerCase()}` : `Wave archive ${c.slot}`;
  }
  function playCry() {
    if (cry) new Audio(asset(`cry/${cry.slot}`, version)).play().catch(() => {});
  }
  let pick = $state(0);
  function useCry(slot: number | null) {
    run(async () => {
      await invoke("set_species_cry", { species, form, slot });
      await reloadCry();
      play("save");
    });
  }
  function newCry() {
    run(async () => {
      const path = await open({ title: "Choose a WAV", filters: [{ name: "WAV sound", extensions: ["wav"] }] });
      if (typeof path !== "string") return;
      const name = form ? `${nameOf(species)} form ${form}` : nameOf(species);
      const s = await invoke<{ seconds: number; rate: number; note: string | null }>("import_new_cry", { species, form, name, path });
      await reloadCry();
      play("save");
      toast(`Imported cry: ${s.seconds.toFixed(2)} s at ${s.rate} Hz${s.note ? `. ${s.note}` : ""}`);
    });
  }
  const species649 = $derived(Array.from({ length: 649 }, (_, k) => ({ i: k + 1, name: nameOf(k + 1) })).sort((a, b) => a.name.localeCompare(b.name)));
</script>

<div class="cols">
  {#if isNew}
    <div class="window block">
      <h3>Forms</h3>
      <ul class="forms">
        <li class:here={form === 0}>
          <span>Base form</span>
          {#if form !== 0}<button class="quiet" onclick={() => onOpen(species)}>Open</button>{/if}
        </li>
        {#each formSlots as slot, i}
          <li class:here={form === i + 1}>
            <span>Form {i + 1}</span>
            {#if form !== i + 1}
              <button class="quiet" onclick={() => onOpen(slot)}>Open</button>
            {:else if !readOnly}
              <button class="quiet" disabled={busy} onclick={removeForm}>{confirmRemove ? "Confirm removal" : "Remove"}</button>
            {/if}
          </li>
        {/each}
      </ul>
      {#if !readOnly}
        <div class="row">
          <label class="field">
            <span class="name">Copy from</span>
            <select value={String(source)} onchange={(e) => (source = Number(e.currentTarget.value))}>
              {#each sources as s}<option value={String(s.i)}>{s.name}</option>{/each}
            </select>
          </label>
          <button disabled={busy} onclick={addForm}>Add form</button>
        </div>
      {/if}
    </div>
  {/if}
  <div class="window block">
    <h3>Cry</h3>
    {#if cry}
      <div class="row">
        <span class="now">{cry.slot === 0 ? "No cry" : `${cryLabel(cry)}${cry.slot === cry.default ? " (default)" : ""}`}</span>
        {#if cry.slot !== 0}<button onclick={playCry}>Play</button>{/if}
        {#if !readOnly && cry.slot !== cry.default}
          <button class="quiet" disabled={busy} onclick={() => useCry(null)}>Reset</button>
        {/if}
      </div>
      {#if !readOnly}
        <div class="row">
          <label class="field">
            <span class="name">Cry from</span>
            <select value={String(pick)} onchange={(e) => (pick = Number(e.currentTarget.value))}>
              <option value="0" disabled>Choose a Pokémon</option>
              {#each species649 as s}<option value={String(s.i)}>{s.name}</option>{/each}
            </select>
          </label>
          <button disabled={busy || !pick} onclick={() => useCry(pick)}>Use</button>
          <button disabled={busy} onclick={newCry} title="WAV, up to 2.9 seconds">Import WAV...</button>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(22rem, 1fr));
    gap: 1rem;
    align-items: start;
  }
  .block {
    padding: 0.9rem 1rem;
  }
  h3 {
    margin: 0 0 0.5rem;
  }
  .forms {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .forms li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    min-height: 2rem;
  }
  .forms li + li {
    border-top: 1px solid var(--rim);
  }
  .forms li.here {
    font-weight: 600;
  }
  .row {
    display: flex;
    align-items: flex-end;
    flex-wrap: wrap;
    gap: 0.6rem 0.75rem;
    margin-top: 0.7rem;
  }
  .now {
    align-self: center;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    width: 16rem;
  }
  .field .name {
    font-size: 0.85rem;
    color: var(--muted);
  }
</style>
