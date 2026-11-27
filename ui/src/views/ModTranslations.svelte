<script lang="ts">
  // Every line the mod sets in its own language with its translation per
  // language (core translations.rs, eidos `<bank>.<lang>.msg`). An empty
  // translation leaves the mod's own line. The placeholder is the game's
  // text in that language (core reference.rs) or the mod's line.
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { LANGUAGES, textLang, referenceText, loadLanguages, clearReferences } from "../lib/textLang.svelte";
  import { play } from "../lib/sfx";
  import { toast } from "morphe/menu.svelte";

  let { readOnly, onSaved }: { readOnly: boolean; onSaved: () => void } = $props();

  interface Row {
    arc: number;
    bank: number;
    line: number;
    text: string;
    translation: string | null;
  }
  const others = $derived(LANGUAGES.filter(([l]) => l !== textLang.base));
  let lang = $state("");
  $effect(() => {
    if (!lang || lang === textLang.base) lang = (textLang.current !== textLang.base ? textLang.current : others[0]?.[0]) ?? "fr";
  });
  let rows = $state<Row[]>([]);
  let edits = $state<Record<string, string>>({});
  let hints = $state<Record<string, string>>({});
  let progress = $state<Record<string, [number, number]>>({});
  let missingOnly = $state(false);
  let busy = $state(false);
  const key = (r: Row) => `${r.arc}/${r.bank}/${r.line}`;
  // Lines a translation needs nothing for: names in capitals (written with
  // the name) and sentences eidos builds from the game's own in each
  // language ("X used ...!", "a ...").
  const AUTO = new Set([13, 279, 281, 284, 285, 286]);
  // What a line is, by its bank (a/0/0/2; TextPanel's fields).
  const WHAT: Record<number, string> = {
    70: "Pokemon name",
    260: "Category",
    236: "Pokedex entry (Black)",
    235: "Pokedex entry (White)",
    244: "Form name",
    245: "Pokedex height",
    268: "Pokedex weight",
    203: "Move name",
    202: "Move description",
    54: "Item name",
    280: "Item plural",
    53: "Item description",
    182: "Ability name",
    183: "Ability description",
    199: "Type name",
    190: "Trainer name",
  };
  const what = (r: Row) => (r.arc === 2 ? WHAT[r.bank] : undefined) ?? `${r.arc === 3 ? "Story" : "Text"} ${r.bank}:${r.line}`;

  async function load() {
    const l = lang;
    if (!l) return;
    const all = await Promise.all(others.map(([o]) => invoke<Row[]>("translation_rows", { lang: o })));
    const own = (rs: Row[]) => rs.filter((r) => !(r.arc === 2 && AUTO.has(r.bank)));
    progress = Object.fromEntries(others.map(([o], i) => [o, [own(all[i]).filter((r) => r.translation !== null).length, own(all[i]).length]]));
    const mine = (all[others.findIndex(([o]) => o === l)] ?? []).filter((r) => !(r.arc === 2 && AUTO.has(r.bank)));
    rows = mine;
    edits = {};
    const h: Record<string, string> = {};
    for (const r of mine) {
      const ref = await referenceText(l, r.arc, r.bank);
      const game = ref?.lines[String(r.line)];
      if (game) h[key(r)] = game;
    }
    if (lang === l) hints = h;
  }
  $effect(() => {
    void lang;
    load().catch((e) => toast(String(e)));
  });

  const shown = $derived(rows.filter((r) => !missingOnly || (r.translation === null && !edits[key(r)])));
  const dirty = $derived(Object.keys(edits).length > 0);
  const show = (s: string) => s.replace(/\\n/g, " / ");
  const value = (r: Row) => edits[key(r)] ?? r.translation ?? "";

  async function save() {
    busy = true;
    try {
      const byBank = new Map<string, [number, string][]>();
      for (const r of rows) {
        const k = key(r);
        if (!(k in edits)) continue;
        const b = `${r.arc}/${r.bank}`;
        byBank.set(b, [...(byBank.get(b) ?? []), [r.line, edits[k].replace(/ \/ /g, "\\n")]]);
      }
      for (const [b, lines] of byBank) {
        const [arc, bank] = b.split("/").map(Number);
        await invoke("set_mod_lang_lines", { arc, bank, lang, lines });
      }
      await load();
      onSaved();
      play("save");
      toast("Translations saved");
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function addReference() {
    const path = await open({ filters: [{ name: "ROM", extensions: ["nds"] }] });
    if (typeof path !== "string") return;
    try {
      const l = await invoke<string>("set_reference_rom", { path });
      clearReferences();
      await loadLanguages();
      await load();
      toast(`${LANGUAGES.find(([c]) => c === l)?.[1] ?? l} ROM added`);
    } catch (e) {
      toast(String(e));
    }
  }
  async function removeReference(l: string) {
    await invoke("clear_reference_rom", { lang: l });
    clearReferences();
    await loadLanguages();
    await load();
  }
</script>

<div class="translations">
  <div class="head">
    <div class="sec-tabs" role="tablist">
      {#each others as [code, name]}
        <button role="tab" aria-selected={lang === code} onclick={() => (lang = code)}>
          {name} <span class="muted">{progress[code]?.[0] ?? 0}/{progress[code]?.[1] ?? 0}</span>
        </button>
      {/each}
    </div>
    <label class="check"><input type="checkbox" bind:checked={missingOnly} /> Missing only</label>
    {#if !readOnly}
      <button disabled={!dirty || busy} onclick={() => (edits = {})}>Discard</button>
      <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
    {/if}
  </div>
  <div class="refs">
    {#if textLang.references[lang]}
      <span class="muted">Reference: {textLang.references[lang]}</span>
      <button class="quiet" onclick={() => removeReference(lang)}>Remove</button>
    {:else}
      <span class="muted">Reference: built-in names and descriptions</span>
      <button onclick={addReference}>Add reference ROM...</button>
    {/if}
  </div>
  {#if !rows.length}
    <p class="muted pad">No text changes yet.</p>
  {:else}
    <table>
      <thead><tr><th class="what"></th><th>Mod text</th><th>{LANGUAGES.find(([c]) => c === lang)?.[1]}</th></tr></thead>
      <tbody>
        {#each shown as r (key(r))}
          <tr>
            <td class="what">{what(r)}</td>
            <td class="src">{show(r.text)}</td>
            <td>
              <input
                value={show(value(r))}
                placeholder={show(hints[key(r)] ?? "")}
                readonly={readOnly}
                oninput={(e) => (edits[key(r)] = e.currentTarget.value)}
              />
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<style>
  .translations {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    padding: 0.6rem 1rem 0;
  }
  .head .sec-tabs {
    flex: 1;
  }
  .check {
    display: flex;
    gap: 0.35rem;
    align-items: center;
    font-size: 0.9rem;
  }
  .refs {
    display: flex;
    gap: 0.6rem;
    align-items: center;
    padding: 0.4rem 1rem;
    font-size: 0.85rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    overflow: auto;
    display: block;
    padding: 0 1rem 1rem;
  }
  thead,
  tbody,
  tr {
    display: table;
    width: 100%;
    table-layout: fixed;
  }
  th {
    text-align: left;
    font-weight: 600;
    padding: 0.4rem 0.5rem;
    color: var(--muted);
  }
  td {
    padding: 0.2rem 0.5rem;
    vertical-align: middle;
  }
  .what {
    width: 11rem;
    color: var(--muted);
    font-size: 0.85rem;
  }
  td.src {
    color: var(--muted);
    font-size: 0.9rem;
  }
  td input {
    width: 100%;
  }
  .pad {
    padding: 1rem;
  }
</style>
