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
  import { textLang, translating, referenceText } from "./textLang.svelte";

  type Kind = "moves" | "items" | "abilities" | "species" | "trainers" | "types";
  let {
    kind,
    id,
    form = false,
    readOnly,
    onSaved,
  }: {
    kind: Kind;
    id: number;
    /// The entry is a form slot of a Pokemon the mod adds.
    form?: boolean;
    readOnly: boolean;
    onSaved: () => void;
  } = $props();

  interface Field {
    label: string;
    bank: number;
    multi?: boolean;
    max?: number;
    /// A Pokedex bank of 727 lines (the game's species, then its forms).
    /// A slot the mod adds (715 and up) is line 727 + (slot - 715), as
    /// eidos reads it; the game's own form records have no line here.
    dex?: boolean;
    /// Shown for: a species (default), a form slot ("form"), or both.
    for?: "form" | "both";
  }
  const dexLine = (slot: number) => (slot >= 715 ? 727 + slot - 715 : slot);
  const lineOf = (f: Field) => (f.dex ? dexLine(id) : id);
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
      { label: "Form name", bank: 244, max: 12, dex: true, for: "form" },
      { label: "Pokedex height", bank: 245, max: 10, dex: true, for: "both" },
      { label: "Pokedex weight", bank: 268, max: 12, dex: true, for: "both" },
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

  const fields = $derived(
    FIELDS[kind].filter(
      (f) =>
        (form ? f.for === "form" || f.for === "both" : f.for !== "form") &&
        // The game's own form records (650-714) have no Pokedex line of their own.
        (!f.dex || id <= 649 || id >= 715),
    ),
  );
  let values = $state<string[]>([]);
  let original = $state<string[]>([]);
  // In a translation: the line in the base language, or the game's own
  // text in that language, shown where the translation is empty.
  let hints = $state<string[]>([]);
  let busy = $state(false);
  const lang = $derived(translating() ? textLang.current : null);

  // The display form writes a line break as \n; the boxes show real ones.
  // A packed line (trainer names) shows without its {packed} marker and is
  // packed again on save.
  const PACKED = "{packed}";
  const toBox = (s: string) => (s.startsWith(PACKED) ? s.slice(PACKED.length) : s).replace(/\\n/g, "\n");
  const fromBoxAt = (i: number, s: string) =>
    ((original[i] ?? "").startsWith(PACKED) ? PACKED : "") + s.replace(/\r?\n/g, "\\n");

  $effect(() => {
    const at = id;
    const l = lang;
    const line = (f: Field) => (f.dex ? dexLine(at) : at);
    const base = Promise.all(fields.map((f) => textBank(2, f.bank).then((b) => b[line(f)] ?? "")));
    if (!l) {
      base
        .then((v) => {
          if (id !== at) return;
          original = v;
          values = v.map(toBox);
          hints = [];
        })
        .catch(() => {});
      return;
    }
    Promise.all([
      base,
      Promise.all(fields.map((f) => invoke<Record<string, string>>("mod_lang_lines", { arc: 2, bank: f.bank, lang: l }))),
      Promise.all(fields.map((f) => referenceText(l, 2, f.bank))),
    ])
      .then(([b, mine, refs]) => {
        if (id !== at || lang !== l) return;
        original = fields.map((f, i) => mine[i][String(line(f))] ?? "");
        values = original.map(toBox);
        hints = fields.map((f, i) => toBox(refs[i]?.lines[String(line(f))] ?? b[i]));
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

  // A translation: its lines in that language; names in capitals follow.
  // Lines built from the game's sentences ("X used ...!") are eidos's.
  async function saveTranslation(l: string) {
    const byBank = new Map<number, [number, string][]>();
    const add = (bank: number, line: number, text: string) => byBank.set(bank, [...(byBank.get(bank) ?? []), [line, text]]);
    fields.forEach((f, i) => {
      const v = fromBoxAt(i, values[i]);
      if (v !== original[i]) add(f.bank, lineOf(f), v);
    });
    const name = fromBoxAt(0, values[0]);
    if (!form && name !== original[0]) {
      for (const [bank, per, how] of FOLLOW[kind]) if (how === "caps") for (let k = 0; k < per; k++) add(bank, id * per + k, name.toUpperCase());
    }
    for (const [bank, lines] of byBank) await invoke("set_mod_lang_lines", { arc: 2, bank, lang: l, lines });
  }

  async function save(quiet = false) {
    busy = true;
    if (lang) {
      try {
        await saveTranslation(lang);
        original = values.map((v, i) => fromBoxAt(i, v));
        onSaved();
        if (!quiet) {
          play("save");
          toast("Translation saved");
        }
      } catch (e) {
        if (quiet) throw e;
        toast(String(e));
        play("cancel");
      } finally {
        busy = false;
      }
      return;
    }
    try {
      const lines: [number, number, string][] = [];
      fields.forEach((f, i) => {
        const v = fromBoxAt(i, values[i]);
        if (v !== original[i]) lines.push([f.bank, lineOf(f), v]);
      });
      const oldName = original[0];
      const newName = fromBoxAt(0, values[0]);
      // A form's first field is its form name: nothing follows it.
      if (!form && newName !== oldName && oldName) {
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
        <button disabled={!dirty || busy} onclick={() => (values = original.map(toBox))}>Discard</button>
        <button class="primary" disabled={!dirty || busy} onclick={() => save()}>Save text</button>
      {/if}
    </div>
  {/if}
  <div class="grid">
    {#each fields as f, i}
      <label class="field" class:wide={f.multi}>
        <span class="name">{f.label}{#if f.max}{" "}<span class="muted">(up to {f.max})</span>{/if}</span>
        {#if f.multi}
          <textarea rows="3" bind:value={values[i]} readonly={readOnly} placeholder={hints[i] ?? ""}></textarea>
        {:else}
          <input bind:value={values[i]} maxlength={f.max} readonly={readOnly} placeholder={hints[i] ?? ""} />
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
