<script lang="ts">
  import { onMount } from "svelte";
  import { play } from "../lib/sfx";
  import VirtualList from "../lib/VirtualList.svelte";
  import { debounced, once } from "../lib/cache.svelte";
  import { asset } from "../lib/backend";
  import { invoke } from "@tauri-apps/api/core";
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
  // learnset, a trainer's party) and are shown and saved with it. `single`
  // shows only the `start` entry, without the list.
  let {
    tables,
    readOnly,
    version,
    start,
    single = false,
    only,
    onSaved,
  }: {
    tables: string[];
    readOnly: boolean;
    version: number;
    start?: number;
    single?: boolean;
    /// Show just this section (by order over all tables), without tabs.
    only?: number;
    onSaved: () => void;
  } = $props();

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
      const first = (start !== undefined ? entries.find((e) => e.index === start) : null) ??
        entries.find((e) => e.index > 0) ??
        entries[0];
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
  // In a side panel, one section at a time.
  let secTab = $state(0);
  const allSections = $derived(records.flatMap((r) => r?.sections ?? []));
  const sectionIndex = (ri: number, si: number) =>
    records.slice(0, ri).reduce((n, r) => n + (r?.sections.length ?? 0), 0) + si;
  const query = debounced(() => filter);
  const shown = $derived.by(() => {
    const q = query.value.trim().toLowerCase();
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

  // The picture of the entry, from the ROM's own tables: a trainer's
  // sprite follows its class as it is edited, a species shows its battle
  // sprite, an item its icon.
  const codeTable = (name: string) =>
    once(`table/${name}`, () => fetch(asset(`table/${name}`)).then((r) => (r.ok ? r.json() : [])));
  let trainerGraphic = $state<number[]>([]);
  let itemIcons = $state<number[]>([]);
  onMount(() => {
    if (tables[0] === "trainers") codeTable("trainer_graphic").then((t) => (trainerGraphic = t));
    if (tables[0] === "items") codeTable("item_icons").then((t) => (itemIcons = t));
  });

  function fieldValue(key: string): number | null {
    for (const s of records[0]?.sections ?? [])
      if (s.kind === "fields") {
        const v = s.values.find((v) => v.key === key);
        if (v) return Number(v.value);
      }
    return null;
  }

  // Where a trainer stands, from the maps' event data.
  interface Location {
    zone: number;
    place: string;
    tile: [number, number] | null;
    pair: boolean;
  }
  let locations = $state<Location[] | null>(null);
  $effect(() => {
    const index = current;
    locations = null;
    if (tables[0] !== "trainers" || index === null) return;
    invoke<Location[]>("trainer_locations", { trainer: index })
      .then((l) => {
        if (current === index) locations = l;
      })
      .catch(() => (locations = []));
  });

  // A small map around the trainer's first spot, from the in-game render.
  const MINI_PX = 8;
  let mini = $state<{ zone: number; x: number; z: number; w: number; h: number } | null>(null);
  $effect(() => {
    const l = locations?.find((l) => l.tile);
    mini = null;
    if (!l || !l.tile) return;
    const at = l.tile;
    fetch(asset(`maprender/${l.zone}/info?px=${MINI_PX}`))
      .then((r) => (r.ok ? r.json() : null))
      .then((info) => {
        if (info) mini = { zone: l.zone, x: at[0] - info.originX, z: at[1] - info.originZ, w: info.width, h: info.height };
      })
      .catch(() => {});
  });

  const portrait = $derived.by(() => {
    if (current === null) return null;
    switch (tables[0]) {
      case "trainers": {
        const cls = fieldValue("trainerClass");
        const g = cls === null ? undefined : trainerGraphic[cls];
        return g === undefined ? null : asset(`preview/72:${8 * g}/72:${8 * g + 7}?cells=2`);
      }
      case "species":
        return current > 0 && current < 650 ? asset(`preview/4:${20 * current}/4:${20 * current + 18}?cells=4`) : null;
      case "items": {
        const at = current * 4;
        if (at + 3 >= itemIcons.length) return null;
        const tile = itemIcons[at] | (itemIcons[at + 1] << 8);
        const pal = itemIcons[at + 2] | (itemIcons[at + 3] << 8);
        return asset(`preview/25:${tile}/25:${pal}?cells=1`);
      }
      default:
        return null;
    }
  });

  const label = (enumName: string | null, value: number) => {
    const list = enumName ? names[enumName] : null;
    return list?.[value] ?? "";
  };
</script>

<div class="tables" class:single>
  {#if !single}
  <aside class="entries">
    <input class="search" placeholder="Search {entries.length}" bind:value={filter} />
    <VirtualList items={shown} height={34} key={(e) => e.index} label="Entries">
      {#snippet row(e)}
        <button class="row-item" aria-current={e.index === current} onclick={() => open(e.index)}>
          <span class="num muted">{e.index}</span>
          <span class="label">{e.name}</span>
        </button>
      {/snippet}
      {#snippet empty()}
        <p class="muted pad">{error ?? (entries.length ? "Nothing matches." : "Loading...")}</p>
      {/snippet}
    </VirtualList>
  </aside>
  {/if}

  <section class="record">
    <div class="head">
      {#if portrait}<img class="px portrait" src={portrait} alt="" />{/if}
      {#if mini}
        <div class="mini" title="Where this trainer stands">
          <img
            src={asset(`maprender/${mini.zone}?px=${MINI_PX}`)}
            alt=""
            style="width: {mini.w * MINI_PX}px; left: {48 - (mini.x + 0.5) * MINI_PX}px; top: {32 - (mini.z + 0.5) * MINI_PX}px"
          />
          <span class="pin"></span>
        </div>
      {/if}
      <div class="title">
        <h2>{entries.find((e) => e.index === current)?.name ?? ""}</h2>
        {#if tables[0] === "trainers" && locations}
          <p class="muted where">
            {#if locations.length === 0}
              No map position
            {:else}
              {#each locations as l, i}
                {i > 0 ? " \u00b7 " : ""}{l.place}{l.tile ? ` (tile ${l.tile[0]}, ${l.tile[1]})` : ""}{l.pair ? ", double battle" : ""}
              {/each}
            {/if}
          </p>
        {/if}
      </div>
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

    {#if single && only === undefined && allSections.length > 1}
      <div class="sec-tabs" role="tablist">
        {#each allSections as s, i}
          <button role="tab" aria-selected={secTab === i} class:primary={secTab === i} onclick={() => (secTab = i)}>{s.label}</button>
        {/each}
      </div>
    {/if}
    <div class="scroll body">
      {#each records as r, ri}
        {#each r?.sections ?? [] as s, si}
          {#if !single || allSections.length < 2 || sectionIndex(ri, si) === (only ?? secTab)}
          <div class="window block">
            {#if !single}<h3>{s.label}</h3>{/if}
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
            {:else if single}
              <!-- Narrow: one small form per row instead of a wide table. -->
              <div class="rows">
                {#each s.rows as row, ri}
                  <div class="row-card">
                    <div class="row-head">
                      <strong>{ri + 1}</strong>
                      {#if !s.fixed && !readOnly}
                        <button class="quiet" onclick={() => s.rows.splice(ri, 1)} aria-label="Remove row">Remove</button>
                      {/if}
                    </div>
                    <div class="grid">
                      {#each s.columns as c, ci}
                        <label class="field">
                          <span class="name">{c.label}</span>
                          {#if c.enum && names[c.enum]?.length}
                            <select bind:value={row[ci]} disabled={readOnly}>
                              {#each names[c.enum] as n, i}<option value={i}>{n || `(${i})`}</option>{/each}
                              {#if row[ci] >= names[c.enum].length}<option value={row[ci]}>({row[ci]})</option>{/if}
                            </select>
                          {:else}
                            <input type="number" min={c.min} max={c.max} bind:value={row[ci]} readonly={readOnly} />
                          {/if}
                        </label>
                      {/each}
                    </div>
                  </div>
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
          {/if}
        {/each}
      {/each}
    </div>
  </section>
</div>

<style>
  .tables {
    display: grid;
    grid-template-columns: 15rem minmax(0, 1fr);
    height: 100%;
  }
  .tables.single {
    grid-template-columns: minmax(0, 1fr);
    height: auto;
    overflow: hidden;
  }
  /* Inside another view's side panel: flows with the panel, no own scroll,
     narrow fields, nothing wider than the panel. */
  .single .record {
    display: block;
  }
  .single .head {
    flex-wrap: wrap;
    padding: 0 0 0.5rem;
    gap: 0.5rem;
  }
  .single .head h2 {
    font-size: 1rem;
  }
  .single .scroll {
    overflow: visible;
  }
  .single .body {
    padding: 0.5rem 0 0;
    gap: 0.6rem;
  }
  .single .block {
    padding: 0;
    border: none;
    box-shadow: none;
    background: none;
    gap: 0.4rem;
  }
  .single .block h3 {
    margin: 0;
    font-size: 0.9rem;
  }
  .single .grid {
    grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr));
    gap: 0.4rem 0.6rem;
  }
  .single .field .name {
    font-size: 0.78rem;
  }
  .field select,
  .field input[type="number"] {
    min-width: 0;
    max-width: 100%;
  }
  .sec-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin-bottom: 0.3rem;
  }
  .sec-tabs button {
    padding: 0.15rem 0.55rem;
    font-size: 0.8rem;
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .row-card {
    border: 2px solid color-mix(in srgb, var(--outline) 50%, transparent);
    border-radius: var(--radius-s);
    padding: 0.4rem 0.5rem;
  }
  .row-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.3rem;
    font-size: 0.85rem;
  }
  .row-head button {
    padding: 0.1rem 0.5rem;
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
  .portrait {
    height: 64px;
    width: auto;
    max-width: 96px;
    object-fit: contain;
    background: var(--bg);
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
  }
  .mini {
    position: relative;
    flex: none;
    width: 96px;
    height: 64px;
    overflow: hidden;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    background: var(--bg);
  }
  .mini img {
    position: absolute;
    max-width: none;
  }
  .pin {
    position: absolute;
    left: 42px;
    top: 26px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #ff4d4d;
    border: 2px solid #fff;
    box-shadow: 0 0 0 2px #0008;
  }
  .title {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .where {
    font-size: 0.85rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
  .pad {
    padding: 1rem;
  }
</style>
