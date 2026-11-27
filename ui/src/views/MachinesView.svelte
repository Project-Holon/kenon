<script lang="ts">
  // Which move each TM and HM teaches (core machines.rs; eidos
  // machine_moves.txt). The game's 101 and the mod's new TMs on one screen;
  // changing one also rewrites
  // that TM's item description to the new move's. Who can learn each one
  // stays on the Pokemon's own TMs and tutors tab.
  import { invoke } from "@tauri-apps/api/core";
  import SearchSelect from "morphe/SearchSelect.svelte";
  import { toast } from "morphe/menu.svelte";
  import { enumValues } from "../lib/tables";
  import { moveIcon, loadIcons } from "../lib/icons.svelte";
  import { play } from "../lib/sfx";

  let { version, readOnly, onSaved }: { version: number; readOnly: boolean; onSaved: () => void } = $props();

  interface Machine {
    label: string;
    item: number;
    mv: number;
    gameMove: number | null;
  }
  let list = $state<Machine[]>([]);
  let edits = $state<number[]>([]);
  let moves = $state<string[]>([]);
  let filter = $state("");
  let saving = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    void version;
    loadIcons();
    enumValues("move").then((m) => (moves = m));
    invoke<Machine[]>("machines")
      .then((l) => {
        list = l;
        edits = l.map((m) => m.mv);
      })
      .catch((e) => (error = String(e)));
  });

  const dirty = $derived(edits.some((m, i) => m !== list[i]?.mv));
  const order = (i: number) => (i >= 95 && i < 101 ? 1000 + i : i);
  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    return list
      .map((m, i) => ({ m, i }))
      .filter(({ m, i }) => !q || m.label.toLowerCase().includes(q) || (moves[edits[i]] ?? "").toLowerCase().includes(q))
      // TM01-TM95, then a mod's TM96 on, then the HMs (slots 95-100).
      .sort((a, b) => order(a.i) - order(b.i));
  });

  // New TMs (TM96 on): one at a time, after the game's; only the last can
  // be removed, so the numbers stay in a row.
  async function reload() {
    list = await invoke<Machine[]>("machines");
    edits = list.map((m) => m.mv);
  }
  async function addTm() {
    saving = true;
    try {
      // It starts on Pound; pick its move in its row.
      const i = await invoke<number>("add_machine", { mv: 1 });
      await reload();
      onSaved();
      play("save");
      toast(`Added ${list[i].label}. Choose its move, then give it to Pokemon on their TMs and tutors tab.`);
    } catch (e) {
      toast(String(e));
    } finally {
      saving = false;
    }
  }
  async function removeTm() {
    saving = true;
    try {
      const l = await invoke<string>("remove_last_machine");
      await reload();
      onSaved();
      play("save");
      toast(`Removed ${l}`);
    } catch (e) {
      toast(String(e));
    } finally {
      saving = false;
    }
  }
  const GAME_MACHINES = 101;

  async function save() {
    saving = true;
    try {
      for (let i = 0; i < list.length; i++) if (edits[i] !== list[i].mv) await invoke("set_machine", { index: i, mv: edits[i] });
      play("save");
      toast("Saved");
      onSaved();
      list = await invoke<Machine[]>("machines");
      edits = list.map((m) => m.mv);
    } catch (e) {
      toast(String(e));
      play("cancel");
    } finally {
      saving = false;
    }
  }
</script>

<div class="machines">
  <div class="head entry-head">
    <input class="search" placeholder="Search TMs or moves" aria-label="Search TMs or moves" bind:value={filter} onkeydown={(e) => e.key === "Escape" && (filter = "")} />
    <span class="muted">Who can learn each one is on a Pokemon's TMs and tutors tab.</span>
    {#if !readOnly}
      <button disabled={dirty || saving || list.length >= 128} title={dirty ? "Save or discard edits first" : "Adds the next TM after the game's"} onclick={addTm}>New TM</button>
      <div class="acts">
        <button disabled={!dirty || saving} onclick={() => (edits = list.map((m) => m.mv))}>Discard</button>
        <button class="primary" disabled={!dirty || saving} onclick={save}>{saving ? "Saving..." : "Save"}</button>
      </div>
    {/if}
  </div>
  {#if error}<p class="bad pad">{error}</p>{/if}
  <div class="grid">
    {#each shown as { m, i } (i)}
      <div class="row" class:changed={m.gameMove !== null || edits[i] !== m.mv} title={m.gameMove !== null ? `Game's move: ${moves[m.gameMove] ?? m.gameMove}` : ""}>
        <span class="label">{m.label}</span>
        <div class="pick">
          <SearchSelect bind:value={edits[i]} options={moves} icon={moveIcon} disabled={readOnly} label={m.label} />
        </div>
        {#if !readOnly && i >= GAME_MACHINES && i === list.length - 1}
          <button class="quiet x" disabled={dirty || saving} onclick={removeTm} aria-label="Remove {m.label}" title="Remove">×</button>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .machines {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .search {
    width: min(18rem, 100%);
  }
  .acts {
    margin-left: auto;
    display: flex;
    gap: 0.5rem;
  }
  .grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(12.5rem, 1fr));
    align-content: start;
    gap: 0.2rem 1rem;
    padding: 0.75rem 1rem;
  }
  .row {
    display: grid;
    grid-template-columns: 3.2rem minmax(0, 1fr);
    align-items: center;
    gap: 0.5rem;
  }
  .row:has(.x) {
    grid-template-columns: 3.2rem minmax(0, 1fr) auto;
  }
  .label {
    font-weight: 700;
    font-size: 0.85rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .row.changed .label {
    color: var(--gold);
  }
  .pick {
    min-width: 0;
  }
  /* Compact rows: every TM and HM on one screen. */
  .pick :global(.current) {
    padding: 0.15rem 0.5rem;
    font-size: 0.9rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
