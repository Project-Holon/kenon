<script lang="ts">
  // The name and texts of a move, item, ability or Pokemon, edited where
  // the entry is (a/0/0/2 banks; core catalog::set_text_lines). Renaming
  // also rewrites the lines that repeat the name: capitals, "X used ...!",
  // "a ...". Banks checked in the ROM (Pokedex entries: Black's overlay 206
  // table loads 236, White's 235).
  import { invoke } from "@tauri-apps/api/core";
  import { textBank } from "./backend";
  import { play } from "./sfx";
  import { toast } from "morphe/menu.svelte";
  import { useParts } from "./parts.svelte";

  type Kind = "moves" | "items" | "abilities" | "species" | "trainers" | "types";
  let {
    kind,
    id,
    readOnly,
    onSaved,
  }: { kind: Kind; id: number; readOnly: boolean; onSaved: () => void } = $props();

  interface Field {
    label: string;
    bank: number;
    multi?: boolean;
    max?: number;
    /// Banks of 727 lines (species and forms): the line of a new Pokemon
    /// is not known yet, so only the game's own species show it.
    gameOnly?: boolean;
  }
  // Lines that follow the name: [bank, lines per id, how].
  type Follow = [number, number, "caps" | "replace"];
  const FIELDS: Record<Kind, Field[]> = {
    trainers: [{ label: "Name", bank: 190, max: 12 }],
    types: [{ label: "Name", bank: 199, max: 8 }],
    moves: [
      { label: "Name", bank: 203, max: 16 },
      { label: "Description", bank: 202, multi: true },
    ],
    items: [
      { label: "Name", bank: 54, max: 16 },
      { label: "Plural", bank: 280, max: 16 },
      { label: "Description", bank: 53, multi: true },
    ],
    abilities: [
      { label: "Name", bank: 182, max: 16 },
      { label: "Description", bank: 183, multi: true },
    ],
    species: [
      { label: "Name", bank: 70, max: 10 },
      { label: "Category", bank: 260, max: 16 },
      { label: "Pokedex height", bank: 245, max: 10, gameOnly: true },
      { label: "Pokedex weight", bank: 268, max: 12, gameOnly: true },
      { label: "Pokedex entry (Black)", bank: 236, multi: true },
      { label: "Pokedex entry (White)", bank: 235, multi: true },
    ],
  };
  const FOLLOW: Record<Kind, Follow[]> = {
    trainers: [],
    types: [],
    moves: [
      [286, 1, "caps"],
      [13, 3, "replace"],
    ],
    items: [[279, 1, "replace"]],
    abilities: [[285, 1, "caps"]],
    species: [
      [284, 1, "caps"],
      [281, 1, "replace"],
    ],
  };

  const fields = $derived(FIELDS[kind].filter((f) => !f.gameOnly || id <= 649));
  let values = $state<string[]>([]);
  let original = $state<string[]>([]);
  let busy = $state(false);

  // The display form writes a line break as \n; the boxes show real ones.
  // A packed line (trainer names) shows without its {packed} marker and is
  // packed again on save.
  const PACKED = "{packed}";
  const toBox = (s: string) => (s.startsWith(PACKED) ? s.slice(PACKED.length) : s).replace(/\\n/g, "\n");
  const fromBoxAt = (i: number, s: string) =>
    ((original[i] ?? "").startsWith(PACKED) ? PACKED : "") + s.replace(/\r?\n/g, "\\n");

  $effect(() => {
    const at = id;
    Promise.all(fields.map((f) => textBank(2, f.bank).then((b) => b[at] ?? "")))
      .then((v) => {
        if (id !== at) return;
        original = v;
        values = v.map(toBox);
      })
      .catch(() => {});
  });

  const dirty = $derived(values.some((v, i) => fromBoxAt(i, v) !== original[i]));

  // Inside an entry screen, the screen's Save and Discard cover this panel.
  const parts = useParts();
  $effect(() => {
    if (!parts || readOnly) return;
    return parts.register("text", {
      dirty: () => dirty,
      save: () => save(true),
      discard: () => (values = original.map(toBox)),
    });
  });

  async function save(quiet = false) {
    busy = true;
    try {
      const lines: [number, number, string][] = [];
      fields.forEach((f, i) => {
        const v = fromBoxAt(i, values[i]);
        if (v !== original[i]) lines.push([f.bank, id, v]);
      });
      const oldName = original[0];
      const newName = fromBoxAt(0, values[0]);
      if (newName !== oldName && oldName) {
        for (const [bank, per, how] of FOLLOW[kind]) {
          const bankLines = await textBank(2, bank);
          for (let k = 0; k < per; k++) {
            const at = id * per + k;
            const cur = bankLines[at] ?? "";
            const next =
              how === "caps"
                ? newName.toUpperCase()
                : cur.split(oldName).join(newName).split(oldName.toUpperCase()).join(newName.toUpperCase());
            if (next !== cur) lines.push([bank, at, next]);
          }
        }
      }
      await invoke("set_text_lines", { lines });
      original = values.map((v, i) => fromBoxAt(i, v));
      onSaved();
      if (!quiet) {
        play("save");
        toast("Text saved");
      }
    } catch (e) {
      if (quiet) throw e;
      toast(String(e));
      play("cancel");
    } finally {
      busy = false;
    }
  }
</script>

<div class="window block text">
  <!-- Inside an entry screen the fields need no title row: the header's Save covers them. -->
  {#if !parts}
    <div class="top">
      <h3>Name and text</h3>
      {#if !readOnly}
        <button class="quiet" disabled={!dirty || busy} onclick={() => (values = original.map(toBox))}>Discard</button>
        <button class="primary" disabled={!dirty || busy} onclick={() => save()}>Save text</button>
      {/if}
    </div>
  {/if}
  <div class="grid">
    {#each fields as f, i}
      <label class="field" class:wide={f.multi}>
        <span class="name">{f.label}{#if f.max}{" "}<span class="muted">(up to {f.max})</span>{/if}</span>
        {#if f.multi}
          <textarea rows="3" bind:value={values[i]} readonly={readOnly}></textarea>
        {:else}
          <input bind:value={values[i]} maxlength={f.max} readonly={readOnly} />
        {/if}
      </label>
    {/each}
  </div>
</div>

<style>
  .block {
    padding: 0.9rem 1rem;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .top h3 {
    flex: 1;
    margin: 0;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
    gap: 0.6rem 1rem;
  }
  .top + .grid {
    margin-top: 0.6rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  /* Two columns: the game's lines are short, and the Black and White
     Pokedex entries sit side by side. */
  .field.wide {
    grid-column: span 2;
    max-width: 36rem;
  }
  .name {
    font-size: 0.85rem;
    color: var(--muted);
  }
  textarea {
    resize: vertical;
    font: inherit;
  }
</style>
