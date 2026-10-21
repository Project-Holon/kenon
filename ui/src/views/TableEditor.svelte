<script lang="ts">
  import { onMount } from "svelte";
  import { play } from "../lib/sfx";
  import {
    enumValues,
    saveRecord,
    tableEntries,
    tableRecord,
    type Entry,
    type Section,
    type TableRecord,
  } from "../lib/tables";

  // `tables[0]` names the entries; the others share its index (a species'
  // learnset, a trainer's party) and are shown and saved with it.
  let {
    tables,
    readOnly,
    version,
    onSaved,
  }: { tables: string[]; readOnly: boolean; version: number; onSaved: () => void } = $props();

  const SEASONS = ["Spring", "Summer", "Autumn", "Winter"];

  let entries = $state<Entry[]>([]);
  let current = $state<number | null>(null);
  let records = $state<(TableRecord | null)[]>([]);
  let original = $state<string>("");
  let season = $state(0);
  let filter = $state("");
  let names = $state<Record<string, string[]>>({});
  let error = $state<string | null>(null);
  let busy = $state(false);

  onMount(async () => {
    try {
      entries = await tableEntries(tables[0]);
      const first = entries.find((e) => e.index > 0) ?? entries[0];
      if (first) await open(first.index, false);
    } catch (e) {
      error = String(e);
    }
  });

  $effect(() => {
    void version;
    if (current !== null) reload();
  });

  async function loadNames(rs: (TableRecord | null)[]) {
    const wanted = new Set<string>();
    for (const r of rs)
      for (const s of r?.sections ?? []) {
        if (s.kind === "fields") s.values.forEach((v) => v.enum && wanted.add(v.enum));
        else s.columns.forEach((c) => c.enum && wanted.add(c.enum));
      }
    for (const name of wanted) if (!names[name]) names[name] = await enumValues(name);
  }

  async function reload() {
    if (current === null) return;
    const rs = await Promise.all(tables.map((t) => tableRecord(t, current!, season).catch(() => null)));
    await loadNames(rs);
    records = rs;
    original = JSON.stringify(rs);
  }

  async function open(index: number, sound = true) {
    if (sound) play("select");
    error = null;
    current = index;
    season = 0;
    await reload();
  }

  const dirty = $derived(JSON.stringify(records) !== original);
  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter((e) => e.name.toLowerCase().includes(q) || String(e.index) === q);
  });

  function blank(section: Extract<Section, { kind: "list" }>): number[] {
    return section.columns.map((c) => Math.max(c.min, 0));
  }

  async function save() {
    if (current === null) return;
    busy = true;
    error = null;
    try {
      for (let i = 0; i < tables.length; i++) {
        const r = records[i];
        if (!r) continue;
        const values: Record<string, number> = {};
        const lists: Record<string, number[][]> = {};
        for (const s of r.sections) {
          if (s.kind === "fields") s.values.forEach((v) => (values[v.key] = Number(v.value)));
          else lists[s.key] = s.rows.map((row) => row.map(Number));
        }
        await saveRecord(tables[i], current, { values, lists, season });
      }
      play("save");
      onSaved();
      await reload();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  const label = (enumName: string | null, value: number) => {
    const list = enumName ? names[enumName] : null;
    return list?.[value] ?? "";
  };
</script>

<div class="tables">
  <aside class="entries">
    <input class="search" placeholder="Search {entries.length}" bind:value={filter} />
    <div class="scroll">
      {#each shown as e (e.index)}
        <button class="row-item" aria-current={e.index === current} onclick={() => open(e.index)}>
          <span class="num muted">{e.index}</span>
          <span class="label">{e.name}</span>
        </button>
      {:else}
        <p class="muted pad">{error ?? (entries.length ? "Nothing matches." : "Loading...")}</p>
      {/each}
    </div>
  </aside>

  <section class="record">
    <div class="head">
      <h2>{entries.find((e) => e.index === current)?.name ?? ""}</h2>
      {#if (records[0]?.seasons ?? 1) > 1}
        <select bind:value={season} onchange={reload} aria-label="Season">
          {#each SEASONS.slice(0, records[0]?.seasons ?? 1) as s, i}<option value={i}>{s}</option>{/each}
        </select>
      {/if}
      <div class="spacer"></div>
      {#if records.some((r) => r?.changed)}<span class="tag changed">changed in this mod</span>{/if}
      {#if readOnly}
        <span class="tag">read only</span>
      {:else}
        <button disabled={!dirty || busy} onclick={reload}>Discard</button>
        <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
      {/if}
    </div>
    {#if error}<p class="bad pad">{error}</p>{/if}

    <div class="scroll body">
      {#each records as r}
        {#each r?.sections ?? [] as s}
          <div class="window block">
            <h3>{s.label}</h3>
            {#if s.kind === "fields"}
              <div class="grid">
                {#each s.values as v}
                  <label class="field" title={v.note ?? ""}>
                    <span class="name">
                      {v.label}
                      {#if v.inferred}<span class="tag" title="Meaning inferred, not confirmed in the game's code">?</span>{/if}
                    </span>
                    {#if v.enum && names[v.enum]?.length}
                      <select bind:value={v.value} disabled={readOnly || v.readonly}>
                        {#each names[v.enum] as n, i}<option value={i}>{n || `(${i})`}</option>{/each}
                        {#if v.value >= names[v.enum].length}<option value={v.value}>({v.value})</option>{/if}
                      </select>
                    {:else if v.max === 1 && v.min === 0}
                      <input type="checkbox" checked={v.value === 1} disabled={readOnly || v.readonly} onchange={(e) => (v.value = (e.currentTarget as HTMLInputElement).checked ? 1 : 0)} />
                    {:else}
                      <input type="number" min={v.min} max={v.max} bind:value={v.value} readonly={readOnly || v.readonly} />
                    {/if}
                  </label>
                {/each}
              </div>
            {:else}
              <table>
                <thead>
                  <tr>
                    {#each s.columns as c}<th>{c.label}</th>{/each}
                    {#if !s.fixed && !readOnly}<th></th>{/if}
                  </tr>
                </thead>
                <tbody>
                  {#each s.rows as row, ri}
                    <tr>
                      {#each s.columns as c, ci}
                        <td>
                          {#if c.enum && names[c.enum]?.length}
                            <select bind:value={row[ci]} disabled={readOnly}>
                              {#each names[c.enum] as n, i}<option value={i}>{n || `(${i})`}</option>{/each}
                              {#if row[ci] >= names[c.enum].length}<option value={row[ci]}>({row[ci]})</option>{/if}
                            </select>
                          {:else}
                            <input type="number" min={c.min} max={c.max} bind:value={row[ci]} readonly={readOnly} />
                          {/if}
                        </td>
                      {/each}
                      {#if !s.fixed && !readOnly}
                        <td><button class="quiet" onclick={() => s.rows.splice(ri, 1)} aria-label="Remove row">Remove</button></td>
                      {/if}
                    </tr>
                  {/each}
                </tbody>
              </table>
              {#if !s.fixed && !readOnly && s.rows.length < s.maxRows}
                <button onclick={() => s.rows.push(blank(s))}>Add row</button>
              {/if}
            {/if}
          </div>
        {/each}
      {/each}
      <p class="muted hint">Names come from your ROM. A field marked ? has a meaning inferred, not confirmed in the game's code.</p>
    </div>
  </section>
</div>

<style>
  .tables {
    display: grid;
    grid-template-columns: 15rem minmax(0, 1fr);
    height: 100%;
  }
  .entries {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.75rem;
    min-height: 0;
    border-right: 2px solid var(--outline);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .num {
    width: 2.5rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
    flex: none;
  }
  .row-item[aria-current="true"] .num {
    color: inherit;
  }
  .record {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .head h2 {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }
  .body {
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .block {
    padding: 0.9rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
    gap: 0.6rem 1rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    min-width: 0;
  }
  .field .name {
    color: var(--muted);
    font-size: 0.85rem;
    display: flex;
    gap: 0.3rem;
    align-items: center;
  }
  .field input[type="number"],
  .field select {
    width: 100%;
  }
  .field input[type="checkbox"] {
    width: 1.2rem;
    height: 1.2rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  th {
    text-align: left;
    font-size: 0.8rem;
    color: var(--muted);
    font-weight: 600;
    padding: 0 0.3rem 0.3rem;
  }
  td {
    padding: 0.15rem 0.3rem;
  }
  td input,
  td select {
    width: 100%;
  }
  .hint {
    font-size: 0.85rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
