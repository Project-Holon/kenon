<script lang="ts">
  // A whole table as a grid for bulk editing (core grid.rs): one row per
  // entry, one column per field. Click a header to sort. A cell is picked
  // by click, a range by shift-click; Ctrl+C copies it as tab-separated
  // text and Ctrl+V pastes such text from the picked cell, so a block moves
  // to and from a spreadsheet. Enum fields copy and paste by name.
  import { invoke } from "@tauri-apps/api/core";
  import { enumValues } from "../lib/tables";
  import { play } from "../lib/sfx";
  import { toast } from "morphe/menu.svelte";

  let {
    table,
    readOnly,
    version,
    onSaved,
    onOpen,
    onClose,
  }: {
    table: string;
    readOnly: boolean;
    version: number;
    onSaved: () => void;
    onOpen: (index: number) => void;
    /// Back to the entry list.
    onClose: () => void;
  } = $props();

  interface Column {
    key: string;
    label: string;
    group: string | null;
    enum: string | null;
    min: number;
    max: number;
    readonly: boolean;
    scale: number | null;
  }
  interface Row {
    index: number;
    name: string;
    values: number[];
    changed: boolean;
  }
  let columns = $state<Column[]>([]);
  let rows = $state<Row[]>([]);
  let enums = $state<Record<string, string[]>>({});
  let error = $state("");
  let loading = $state(true);
  // Edits: "<index>:<column>" to value.
  let edits = $state<Record<string, number>>({});

  async function load() {
    loading = true;
    try {
      const g = await invoke<{ columns: Column[]; rows: Row[] } | null>("table_grid", { table });
      if (!g) {
        error = "No table view available.";
        return;
      }
      columns = g.columns;
      rows = g.rows;
      const names = [...new Set(g.columns.map((c) => c.enum).filter((e): e is string => !!e))];
      const lists = await Promise.all(names.map((n) => enumValues(n).catch(() => [] as string[])));
      enums = Object.fromEntries(names.map((n, i) => [n, lists[i]]));
      edits = {};
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    void version;
    void table;
    load();
  });

  // Column groups: one at a time keeps wide tables readable.
  const groups = $derived([...new Set(columns.map((c) => c.group ?? "Other"))]);
  let group = $state("");
  const shownCols = $derived(
    columns.map((c, i) => ({ c, i })).filter(({ c }) => !group || (c.group ?? "Other") === group),
  );

  let filter = $state("");
  let sortCol = $state<number | null>(null);
  let sortDown = $state(false);
  const value = (r: Row, ci: number) => edits[`${r.index}:${ci}`] ?? r.values[ci];
  const shownRows = $derived.by(() => {
    const f = filter.trim().toLowerCase();
    let out = rows.filter((r) => !f || r.name.toLowerCase().includes(f) || String(r.index) === f);
    if (sortCol !== null) {
      const ci = sortCol;
      out = [...out].sort((a, b) => (value(a, ci) - value(b, ci)) * (sortDown ? -1 : 1) || a.index - b.index);
    }
    return out;
  });
  function sortBy(ci: number) {
    if (sortCol === ci) sortDown = !sortDown;
    else {
      sortCol = ci;
      sortDown = false;
    }
  }

  // Only the rows in view are drawn.
  const ROW = 30;
  let scrollTop = $state(0);
  let viewH = $state(600);
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 5));
  const last = $derived(Math.min(shownRows.length, Math.ceil((scrollTop + viewH) / ROW) + 5));

  // Selection: rows and columns as positions in the shown lists.
  let anchor = $state<[number, number] | null>(null);
  let focus = $state<[number, number] | null>(null);
  const inSel = (r: number, c: number) =>
    !!anchor &&
    !!focus &&
    r >= Math.min(anchor[0], focus[0]) &&
    r <= Math.max(anchor[0], focus[0]) &&
    c >= Math.min(anchor[1], focus[1]) &&
    c <= Math.max(anchor[1], focus[1]);
  function pick(e: MouseEvent, r: number, c: number) {
    if (e.shiftKey && anchor) focus = [r, c];
    else anchor = focus = [r, c];
  }

  const label = (col: Column, v: number) => (col.enum ? (enums[col.enum]?.[v] ?? String(v)) : String(v));
  function parse(col: Column, text: string): number | null {
    const t = text.trim();
    if (col.enum) {
      const list = enums[col.enum] ?? [];
      const at = list.findIndex((n) => n.toLowerCase() === t.toLowerCase());
      if (at >= 0) return at;
    }
    const n = Number(t);
    return Number.isFinite(n) && t !== "" ? Math.round(n) : null;
  }
  function set(row: Row, ci: number, v: number) {
    const col = columns[ci];
    if (readOnly || col.readonly) return;
    const clamped = Math.max(col.min, Math.min(col.max, v));
    const key = `${row.index}:${ci}`;
    if (clamped === row.values[ci]) {
      const { [key]: _, ...rest } = edits;
      edits = rest;
    } else edits = { ...edits, [key]: clamped };
  }

  function keydown(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || !anchor || !focus) return;
    const r0 = Math.min(anchor[0], focus[0]);
    const r1 = Math.max(anchor[0], focus[0]);
    const c0 = Math.min(anchor[1], focus[1]);
    const c1 = Math.max(anchor[1], focus[1]);
    if (e.key === "c") {
      e.preventDefault();
      const lines = [];
      for (let r = r0; r <= r1; r++) {
        const row = shownRows[r];
        const cells = [];
        for (let c = c0; c <= c1; c++) {
          const { c: col, i } = shownCols[c];
          cells.push(label(col, value(row, i)));
        }
        lines.push(cells.join("\t"));
      }
      navigator.clipboard.writeText(lines.join("\n"));
      toast(`Copied ${(r1 - r0 + 1) * (c1 - c0 + 1)} cells`);
    } else if (e.key === "v" && !readOnly) {
      e.preventDefault();
      navigator.clipboard.readText().then((text) => {
        const lines = text.replace(/\r/g, "").replace(/\n$/, "").split("\n");
        let count = 0;
        let refused = 0;
        lines.forEach((line, dr) => {
          const row = shownRows[r0 + dr];
          if (!row) return;
          line.split("\t").forEach((cell, dc) => {
            const sc = shownCols[c0 + dc];
            if (!sc) return;
            const v = parse(sc.c, cell);
            if (v === null || sc.c.readonly) refused++;
            else {
              set(row, sc.i, v);
              count++;
            }
          });
        });
        toast(refused ? `Pasted ${count} cells, ${refused} invalid` : `Pasted ${count} cells`);
      });
    }
  }

  const dirtyCount = $derived(Object.keys(edits).length);
  let busy = $state(false);
  async function save() {
    busy = true;
    try {
      const cells = Object.entries(edits).map(([k, v]) => {
        const [index, ci] = k.split(":").map(Number);
        return { index, key: columns[ci].key, value: v };
      });
      const n = await invoke<number>("set_grid_cells", { table, cells });
      await load();
      onSaved();
      play("save");
      toast(`Saved ${n} ${n === 1 ? "entry" : "entries"}`);
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }
</script>

<div class="grid-view" role="presentation" onkeydown={keydown}>
  <div class="bar">
    <button disabled={dirtyCount > 0} title={dirtyCount ? "Save or discard edits first" : "One entry at a time"} onclick={onClose}>List</button>
    <input class="search" placeholder="Search {rows.length} entries" bind:value={filter} />
    {#if groups.length > 1}
      <select bind:value={group} aria-label="Columns">
        <option value="">All columns</option>
        {#each groups as g}<option value={g}>{g}</option>{/each}
      </select>
    {/if}
    <span class="muted hint">Shift-click to select a range. Ctrl+C/Ctrl+V to copy and paste.</span>
    <div class="spacer"></div>
    {#if !readOnly}
      <button disabled={!dirtyCount || busy} onclick={() => (edits = {})}>Discard</button>
      <button class="primary" disabled={!dirtyCount || busy} onclick={save}>{dirtyCount ? `Save (${dirtyCount})` : "Save"}</button>
    {/if}
  </div>
  {#if error}
    <p class="bad pad">{error}</p>
  {:else if loading}
    <p class="muted pad">Loading...</p>
  {:else}
    <div class="scroll" bind:clientHeight={viewH} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
      <table style:height="{(shownRows.length + 1) * ROW}px">
        <thead>
          <tr>
            <th class="name">Name</th>
            {#each shownCols as { c, i }}
              <th class:sorted={sortCol === i} title={c.group ?? ""}>
                <button onclick={() => sortBy(i)}>{c.label}{sortCol === i ? (sortDown ? " ▼" : " ▲") : ""}</button>
              </th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#if first > 0}<tr style:height="{first * ROW}px"><td colspan={shownCols.length + 1}></td></tr>{/if}
          {#each shownRows.slice(first, last) as row, k (row.index)}
            {@const r = first + k}
            <tr>
              <th class="name">
                <button onclick={() => onOpen(row.index)} title="Open">{row.name}</button>
              </th>
              {#each shownCols as { c, i }, ci}
                {@const v = value(row, i)}
                <td
                  class:sel={inSel(r, ci)}
                  class:edited={`${row.index}:${i}` in edits}
                  class:ro={c.readonly}
                  onmousedown={(e) => pick(e, r, ci)}
                >
                  {#if c.enum && enums[c.enum]?.length}
                    <select value={String(v)} disabled={readOnly || c.readonly} onchange={(e) => set(row, i, Number(e.currentTarget.value))}>
                      {#each enums[c.enum] as n, j}<option value={String(j)}>{n || j}</option>{/each}
                    </select>
                  {:else}
                    <input
                      type="number"
                      value={v}
                      min={c.min}
                      max={c.max}
                      readonly={readOnly || c.readonly}
                      onchange={(e) => set(row, i, Number(e.currentTarget.value))}
                    />
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
          {#if last < shownRows.length}<tr style:height="{(shownRows.length - last) * ROW}px"><td colspan={shownCols.length + 1}></td></tr>{/if}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .grid-view {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    height: 100%;
    min-width: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .bar .search {
    width: 14rem;
  }
  .hint {
    font-size: 0.85rem;
  }
  .spacer {
    flex: 1;
  }
  .scroll {
    overflow: auto;
    min-height: 0;
  }
  table {
    border-collapse: separate;
    border-spacing: 0;
    font-size: 0.85rem;
  }
  th,
  td {
    height: 30px;
    padding: 0;
    border-right: 1px solid var(--rim);
    border-bottom: 1px solid var(--rim);
    white-space: nowrap;
  }
  thead th {
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--bg);
  }
  th.name {
    position: sticky;
    left: 0;
    z-index: 1;
    background: var(--bg);
    min-width: 11rem;
    text-align: left;
  }
  thead th.name {
    z-index: 3;
    padding: 0 0.5rem;
  }
  th button {
    width: 100%;
    height: 100%;
    background: none;
    border: none;
    box-shadow: none;
    padding: 0 0.5rem;
    font-size: 0.8rem;
    font-weight: 600;
    text-align: left;
    color: inherit;
  }
  th.sorted {
    color: var(--accent-dark);
  }
  td input,
  td select {
    width: 100%;
    min-width: 4.5rem;
    height: 100%;
    border: none;
    border-radius: 0;
    background: transparent;
    padding: 0 0.4rem;
    font-size: 0.85rem;
  }
  td select {
    min-width: 8rem;
  }
  td.sel {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  td.edited {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  td.ro {
    color: var(--muted);
  }
  .pad {
    padding: 1rem;
  }
  .bad {
    color: var(--bad);
  }
</style>
