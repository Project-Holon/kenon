<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import { play } from "../lib/sfx";
  import VirtualList from "../lib/VirtualList.svelte";
  import SearchSelect from "morphe/SearchSelect.svelte";
  import { enumIcon, itemIcon, loadIcons, moveIcon, speciesIcon, trainerClassIcon, trainerIcon } from "../lib/icons.svelte";
  import { debounced } from "morphe/cache.svelte";
  import { asset, changedMembers, revert } from "../lib/backend";
  import { copyText, guarded, openInFiles, saveMember } from "../lib/exporter";
  import { keepNative, openMenu, toast } from "morphe/menu.svelte";
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
  // Lists longer than this get a search box.
  const LONG = 30;

  let entries = $state<Entry[]>([]);
  let current = $state<number | null>(null);
  let records = $state<(TableRecord | null)[]>([]);
  let original = $state<string>("");
  let season = $state(0);
  let filter = $state("");
  let names = $state<Record<string, string[]>>({});
  let error = $state<string | null>(null);
  let busy = $state(false);
  let loading = $state(false);
  let list = $state<{ reveal: (index: number) => void } | null>(null);
  // Unsaved edits of entries left behind, so switching entries loses nothing.
  const drafts = new SvelteMap<number, { records: (TableRecord | null)[]; season: number }>();
  let seq = 0;

  onMount(async () => {
    loadIcons();
    try {
      entries = await tableEntries(tables[0]);
      const first = (start !== undefined ? entries.find((e) => e.index === start) : null) ??
        entries.find((e) => e.index > 0) ??
        entries[0];
      if (first) await open(first.index, false);
      if (current !== null) list?.reveal(entries.findIndex((e) => e.index === current));
    } catch (e) {
      error = String(e);
    }
  });

  $effect(() => {
    void version;
    untrack(() => {
      if (current !== null) reload();
    });
  });

  async function loadNames(rs: (TableRecord | null)[]) {
    const wanted = new Set<string>();
    for (const r of rs)
      for (const s of r?.sections ?? []) {
        if (s.kind === "fields") s.values.forEach((v) => v.enum && wanted.add(v.enum));
        else s.columns.forEach((c) => c.enum && wanted.add(c.enum));
      }
    await Promise.all(
      [...wanted].filter((n) => !names[n]).map(async (n) => (names[n] = await enumValues(n))),
    );
  }

  async function reload(draft?: { records: (TableRecord | null)[]; season: number }) {
    if (current === null) return;
    const my = ++seq;
    const index = current;
    loading = true;
    let failed: string | null = null;
    const rs = await Promise.all(
      tables.map((t) =>
        tableRecord(t, index, season).catch((e) => {
          failed = String(e);
          return null;
        }),
      ),
    );
    await loadNames(draft ? draft.records : rs);
    // A newer request (another entry clicked) owns the screen now.
    if (my !== seq) return;
    loading = false;
    original = JSON.stringify(rs);
    records = draft ? draft.records : rs;
    if (draft) season = draft.season;
    if (rs.every((r) => r === null)) error = failed ?? "Could not read this entry.";
  }

  async function open(index: number, sound = true) {
    if (index === current) return;
    if (sound) play("select");
    if (current !== null && dirty) drafts.set(current, { records: $state.snapshot(records) as (TableRecord | null)[], season });
    else if (current !== null) drafts.delete(current);
    error = null;
    current = index;
    const draft = drafts.get(index);
    drafts.delete(index);
    season = draft?.season ?? 0;
    await reload(draft);
  }

  // Up and down in the list or the search box step through the entries.
  function step(e: KeyboardEvent) {
    const d = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (!d || !shown.length) return;
    e.preventDefault();
    const at = shown.findIndex((x) => x.index === current);
    const to = Math.max(0, Math.min(shown.length - 1, at < 0 ? 0 : at + d));
    open(shown[to].index);
    list?.reveal(to);
  }

  function keys(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      if (!readOnly && dirty && !busy) save();
    }
  }

  // Typed numbers stay inside the field's range.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function fix(obj: any, key: string | number, min: number, max: number) {
    const raw = obj[key];
    let n = raw === null || raw === "" ? Math.max(min, 0) : Math.round(Number(raw));
    if (!Number.isFinite(n)) n = Math.max(min, 0);
    obj[key] = Math.min(max, Math.max(min, n));
  }

  const dirty = $derived(JSON.stringify(records) !== original);
  // One section at a time (a species' stats, then its learnset...).
  let secTab = $state(0);
  const allSections = $derived(records.flatMap((r) => r?.sections ?? []));
  const activeTab = $derived(Math.min(secTab, Math.max(0, allSections.length - 1)));
  const tabbed = $derived(only === undefined && allSections.length > 1);
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
      drafts.delete(current);
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
        return cls === null ? null : trainerClassIcon(cls);
      }
      case "species":
        return current > 0 && current < 650 ? asset(`preview/4:${20 * current}/4:${20 * current + 18}?cells=4`) : null;
      case "items":
        return itemIcon(current);
      default:
        return null;
    }
  });

  // The small picture of a row in the list.
  const rowIcon = (index: number): string | null => {
    switch (tables[0]) {
      case "species":
        return speciesIcon(index);
      case "items":
        return itemIcon(index);
      case "trainers":
        return trainerIcon(index);
      case "moves":
        return moveIcon(index);
      default:
        return null;
    }
  };
  const hasIcons = $derived(["species", "items", "trainers", "moves"].includes(tables[0]));
  const edited = (index: number) => drafts.has(index) || (index === current && dirty);

  // The archive of each table, and which of its members the mod changes.
  let archiveIds = $state<number[]>([]);
  let changedSets = $state<Set<number>[]>([]);
  onMount(() => {
    Promise.all(tables.map((t) => invoke<number>("table_archive", { table: t })))
      .then((ids) => (archiveIds = ids))
      .catch(() => {});
  });
  $effect(() => {
    void version;
    const ids = archiveIds;
    Promise.all(ids.map((a) => changedMembers(a).catch(() => [] as number[])))
      .then((all) => (changedSets = all.map((c) => new Set(c))))
      .catch(() => {});
  });
  const isChanged = (index: number) => changedSets.some((s) => s.has(index));

  const nameOf = (index: number) => entries.find((e) => e.index === index)?.name ?? `Entry ${index}`;

  // The values of an entry as text: one line per field, one per list row.
  function recordText(index: number, rs: (TableRecord | null)[]): string {
    const show = (v: number, en: string | null) => (en && names[en]?.[v]) || String(v);
    const out = [nameOf(index)];
    for (const s of rs.flatMap((r) => r?.sections ?? [])) {
      out.push("", s.label);
      if (s.kind === "fields") for (const v of s.values) out.push(`${v.label}: ${show(Number(v.value), v.enum)}`);
      else {
        out.push(s.columns.map((c) => c.label).join("\t"));
        for (const row of s.rows) out.push(row.map((v, i) => show(Number(v), s.columns[i].enum)).join("\t"));
      }
    }
    return out.join("\n");
  }

  async function copyValues(index: number) {
    if (index === current) return copyText(recordText(index, $state.snapshot(records) as (TableRecord | null)[]), "Copied values");
    const rs = await Promise.all(tables.map((t) => tableRecord(t, index, 0).catch(() => null)));
    await loadNames(rs);
    await copyText(recordText(index, rs), "Copied values");
  }

  async function revertEntry(index: number) {
    for (let i = 0; i < tables.length; i++) if (changedSets[i]?.has(index)) await revert([archiveIds[i], index]);
    drafts.delete(index);
    toast("Reverted to the game's version");
    onSaved();
  }

  // `table` picks which of the entry's files to save (the first by default).
  function entryMenu(e: MouseEvent, index: number, table = 0) {
    if (keepNative(e)) return;
    const arc = archiveIds[table];
    openMenu(e, [
      { label: "Copy name", run: () => copyText(nameOf(index)) },
      { label: "Copy values as text", run: guarded(() => copyValues(index)) },
      "-",
      {
        label: "Save game file",
        disabled: arc === undefined,
        run: guarded(() => saveMember(arc, index, nameOf(index))),
      },
      { label: "Open in Files", disabled: arc === undefined, run: () => openInFiles(arc, index) },
      ...(isChanged(index) && !readOnly ? ["-" as const, { label: "Revert entry to game", run: guarded(() => revertEntry(index)) }] : []),
    ]);
  }

  // On the record: the file of the section shown.
  function recordMenu(e: MouseEvent) {
    if (current === null) return;
    let table = 0;
    if (only !== undefined || tabbed) {
      const at = only ?? activeTab;
      let n = 0;
      records.forEach((r, ri) => {
        const k = r?.sections.length ?? 0;
        if (at >= n && at < n + k) table = ri;
        n += k;
      });
    }
    entryMenu(e, current, table);
  }

  const sectionLabel = (s: Section) => (s.kind === "list" ? `${s.label} (${s.rows.length})` : s.label);
</script>

{#snippet pick(obj: any, key: string | number, en: string | null, min: number, max: number, ro: boolean, name: string)}
  {#if en && names[en]?.length}
    {#if names[en].length > LONG}
      <SearchSelect bind:value={obj[key]} options={names[en]} icon={enumIcon(en)} disabled={ro} label={name} />
    {:else}
      <select bind:value={obj[key]} disabled={ro} aria-label={name}>
        {#each names[en] as n, i}<option value={i}>{n || `(${i})`}</option>{/each}
        {#if obj[key] >= names[en].length}<option value={obj[key]}>({obj[key]})</option>{/if}
      </select>
    {/if}
  {:else}
    <input type="number" {min} {max} bind:value={obj[key]} onchange={() => fix(obj, key, min, max)} readonly={ro} aria-label={name} />
  {/if}
{/snippet}

<div class="tables" class:single role="presentation" onkeydown={keys}>
  {#if !single}
  <aside class="entries" onkeydown={step} role="presentation">
    <input class="search" placeholder="Search {entries.length} entries" aria-label="Search entries" bind:value={filter} onkeydown={(e) => e.key === "Escape" && (filter = "")} />
    <VirtualList bind:this={list} items={shown} height={34} key={(e) => e.index} label="Entries">
      {#snippet row(e)}
        <button class="row-item" aria-current={e.index === current} onclick={() => open(e.index)} oncontextmenu={(ev) => entryMenu(ev, e.index)}>
          <span class="num muted">{e.index}</span>
          {#if hasIcons}
            {@const src = rowIcon(e.index)}
            {#if src}<img class="px ico" {src} alt="" loading="lazy" />{:else}<span class="ico"></span>{/if}
          {/if}
          <span class="label">{e.name}</span>
          {#if edited(e.index)}<span class="tag changed" title="Not saved yet">edited</span>{/if}
        </button>
      {/snippet}
      {#snippet empty()}
        <p class="muted pad">{error ?? (entries.length ? "Nothing matches." : "Loading...")}</p>
      {/snippet}
    </VirtualList>
  </aside>
  {/if}

  <section class="record" role="presentation" oncontextmenu={recordMenu}>
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
        <select bind:value={season} onchange={() => reload()} aria-label="Season">
          {#each SEASONS.slice(0, records[0]?.seasons ?? 1) as s, i}<option value={i}>{s}</option>{/each}
        </select>
      {/if}
      <div class="spacer"></div>
      {#if records.some((r) => r?.changed)}<span class="tag changed">changed in this mod</span>{/if}
      {#if readOnly}
        <span class="tag">read only</span>
      {:else}
        <button class="act" disabled={!dirty || busy} onclick={() => reload()}>Discard</button>
        <button class="act primary" disabled={!dirty || busy} onclick={save}>Save</button>
      {/if}
    </div>
    {#if error}<p class="bad pad">{error}</p>{/if}

    {#if tabbed}
      <div class="sec-tabs" role="tablist">
        {#each allSections as s, i}
          <button role="tab" aria-selected={activeTab === i} class:primary={activeTab === i} onclick={() => (secTab = i)}>{sectionLabel(s)}</button>
        {/each}
      </div>
    {/if}
    <div class="scroll body">
      {#if !records.length && loading}
        <p class="muted">Loading...</p>
      {/if}
      {#each records as r, ri}
        {#each r?.sections ?? [] as s, si}
          {#if only !== undefined ? sectionIndex(ri, si) === only : !tabbed || sectionIndex(ri, si) === activeTab}
          <div class="window block">
            {#if !single && !tabbed}<h3>{s.label}</h3>{/if}
            {#if s.kind === "fields"}
              <div class="grid">
                {#each s.values as v}
                  <label class="field" title={v.note ?? ""}>
                    <span class="name">
                      {v.label}
                      {#if v.inferred}<span class="tag" title="Meaning inferred, not confirmed in the game's code">?</span>{/if}
                    </span>
                    {#if !(v.enum && names[v.enum]?.length) && v.max === 1 && v.min === 0}
                      <input type="checkbox" checked={v.value === 1} disabled={readOnly || v.readonly} onchange={(e) => (v.value = (e.currentTarget as HTMLInputElement).checked ? 1 : 0)} />
                    {:else}
                      {@render pick(v, "value", v.enum, v.min, v.max, readOnly || v.readonly, v.label)}
                    {/if}
                  </label>
                {/each}
              </div>
            {:else}
              {#if s.rows.length === 0}<p class="muted">Nothing here yet.</p>{/if}
              {#if single}
                <!-- Narrow: one small form per row instead of a wide table. -->
                <div class="rows">
                  {#each s.rows as row, rn}
                    <div class="row-card">
                      <div class="row-head">
                        <strong>{rn + 1}</strong>
                        {#if !s.fixed && !readOnly}
                          <button class="quiet" onclick={() => s.rows.splice(rn, 1)} aria-label="Remove row {rn + 1}">Remove</button>
                        {/if}
                      </div>
                      <div class="grid">
                        {#each s.columns as c, ci}
                          <label class="field">
                            <span class="name">{c.label}</span>
                            {@render pick(row, ci, c.enum, c.min, c.max, readOnly, c.label)}
                          </label>
                        {/each}
                      </div>
                    </div>
                  {/each}
                </div>
              {:else if s.rows.length}
                <table>
                  <thead>
                    <tr>
                      {#each s.columns as c}
                        <th style:width={c.enum && names[c.enum]?.length ? undefined : "5.5rem"}>{c.label}</th>
                      {/each}
                      {#if !s.fixed && !readOnly}<th style="width: 5.5rem"></th>{/if}
                    </tr>
                  </thead>
                  <tbody>
                    {#each s.rows as row, rn}
                      <tr>
                        {#each s.columns as c, ci}
                          <td>{@render pick(row, ci, c.enum, c.min, c.max, readOnly, c.label)}</td>
                        {/each}
                        {#if !s.fixed && !readOnly}
                          <td><button class="quiet" onclick={() => s.rows.splice(rn, 1)} aria-label="Remove row {rn + 1}">Remove</button></td>
                        {/if}
                      </tr>
                    {/each}
                  </tbody>
                </table>
              {/if}
              {#if !s.fixed && !readOnly}
                <div class="add">
                  <button disabled={s.rows.length >= s.maxRows} onclick={() => s.rows.push(blank(s))}>Add row</button>
                  <span class="muted">{s.rows.length} of {s.maxRows}</span>
                </div>
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
    grid-template-columns: 17rem minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
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
    display: flex;
    flex-direction: column;
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
  .act {
    flex: none;
  }
  .head,
  .sec-tabs {
    flex: none;
  }
  .ico {
    flex: none;
    width: auto;
    min-width: 24px;
    max-width: 32px;
    height: 24px;
    object-fit: contain;
    image-rendering: pixelated;
  }
  .row-item .label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .add {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .sec-tabs {
    padding: 0.5rem 1rem 0;
  }
  .single .sec-tabs {
    padding: 0;
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
