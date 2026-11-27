<script lang="ts">
  // An added type's plate and drive (core type_items.rs, eidos types.txt):
  // Arceus with Multitype holding the plate takes the type and Judgment
  // follows; Techno Blast takes the drive's. Add makes a new item for the
  // type (a copy of the Flame Plate or the Douse Drive).
  import { invoke } from "@tauri-apps/api/core";
  import SearchSelect from "morphe/SearchSelect.svelte";
  import { enumValues, clearEnums } from "./tables";
  import { itemIcon } from "./icons.svelte";
  import { play } from "./sfx";
  import { toast } from "morphe/menu.svelte";

  let {
    id,
    readOnly,
    version,
    onChanged,
  }: { id: number; readOnly: boolean; version: number; onChanged: () => void } = $props();

  const FIRST_FORM = 5;
  let plate = $state<number | null>(null);
  let drive = $state<[number, number] | null>(null);
  let form = $state(FIRST_FORM);
  let items = $state<string[]>([]);
  let arceusForms = $state(17);
  let busy = $state(false);

  async function load() {
    const look = await invoke<{ plate: number | null; drive: [number, number] | null } | null>("type_look", { id });
    plate = look?.plate ?? null;
    drive = look?.drive ?? null;
    form = drive?.[1] ?? FIRST_FORM;
    items = await enumValues("item");
    arceusForms = await invoke<number>("game_form_count", { species: 493 });
  }
  $effect(() => {
    void id;
    void version;
    load().catch(() => {});
  });

  async function run(what: () => Promise<unknown>, done: string) {
    busy = true;
    try {
      await what();
      clearEnums();
      await load();
      onChanged();
      play("save");
      toast(done);
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }
  const options = $derived<[number, string][]>([[0, "None"], ...items.map((n, i) => [i, n] as [number, string]).slice(1)]);
  const setPlate = (v: number) => run(() => invoke("set_type_plate", { id, item: v || null }), "Plate saved");
  const setDrive = (v: number) => run(() => invoke("set_type_drive", { id, drive: v ? [v, form] : null }), "Drive saved");
  function setForm(f: number) {
    form = f;
    if (drive && f >= FIRST_FORM) setDrive(drive[0]);
  }
</script>

<div class="items">
  <h4>Plate and drive</h4>
  <div class="row">
    <span class="name">Plate</span>
    <SearchSelect value={plate ?? 0} {options} icon={(v) => (v ? itemIcon(v) : null)} disabled={readOnly || busy} label="Plate" onchange={setPlate} />
    {#if !readOnly && plate === null}
      <button disabled={busy} onclick={() => run(() => invoke("add_type_plate", { id }), "Plate added")}>Add plate</button>
    {/if}
  </div>
  <div class="row">
    <span class="name">Drive</span>
    <SearchSelect value={drive?.[0] ?? 0} {options} icon={(v) => (v ? itemIcon(v) : null)} disabled={readOnly || busy} label="Drive" onchange={setDrive} />
    {#if !readOnly && drive === null}
      <button disabled={busy} onclick={() => run(() => invoke("add_type_drive", { id }), "Drive and Genesect form added")}>Add drive</button>
    {/if}
  </div>
  <label class="row">
    <span class="name">Genesect form</span>
    <input type="number" min={FIRST_FORM} max="31" value={form} disabled={readOnly || busy} onchange={(e) => setForm(Number(e.currentTarget.value))} />
  </label>
  <div class="row">
    <span class="name">Arceus form</span>
    {#if arceusForms > id}
      <span>Form {id}</span>
    {:else if !readOnly && arceusForms === id}
      <span class="muted">None</span>
      <button disabled={busy} onclick={() => run(() => invoke("add_arceus_form", { id }), "Arceus form added")}>Add form {id}</button>
    {:else}
      <span class="muted">Add Arceus forms for earlier types first.</span>
    {/if}
  </div>
  <p class="muted note">Recolor new forms in Battle sprites.</p>
</div>

<style>
  .items {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-top: 1rem;
  }
  h4 {
    margin: 0;
  }
  .row {
    display: grid;
    grid-template-columns: 7rem minmax(0, 1fr) auto;
    gap: 0.5rem;
    align-items: center;
  }
  .name {
    color: var(--muted);
    font-size: 0.9rem;
  }
  input[type="number"] {
    width: 5rem;
  }
  .note {
    margin: 0;
    font-size: 0.85rem;
  }
</style>
