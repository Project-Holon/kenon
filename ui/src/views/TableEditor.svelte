<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import { play } from "../lib/sfx";
  import VirtualList from "../lib/VirtualList.svelte";
  import SearchSelect from "morphe/SearchSelect.svelte";
  import { enumIcon, itemIcon, loadIcons, moveIcon, reloadIcons, speciesIcon, trainerClassIcon, trainerIcon } from "../lib/icons.svelte";
  import { debounced, forget } from "morphe/cache.svelte";
  import ContentDialog from "../lib/ContentDialog.svelte";
  import EffectSummary from "../lib/EffectSummary.svelte";
  import ExpCurve from "../lib/ExpCurve.svelte";
  import IconPicker from "../lib/IconPicker.svelte";
  import MemberExtras, { type Extras } from "../lib/MemberExtras.svelte";
  import { tip } from "../lib/tip";
  import { provideParts } from "../lib/parts.svelte";
  import BattleHandler from "../lib/BattleHandler.svelte";
  import MoveAnim from "../lib/MoveAnim.svelte";
  import MoveUsers from "../lib/MoveUsers.svelte";
  import ItemUsers from "../lib/ItemUsers.svelte";
  import TextPanel from "../lib/TextPanel.svelte";
  import MachineGrid from "../lib/MachineGrid.svelte";

  // The species fields MachineGrid shows as checkboxes.
  const MACHINE_KEYS = ["tmFlags1", "tmFlags2", "tmFlags3", "tmFlags4", "tutorFlags"];
  import { asset, changedMembers, revert } from "../lib/backend";
  import { copyText, guarded, openInFiles, saveMember } from "../lib/exporter";
  import { keepNative, openMenu, toast } from "morphe/menu.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { open as openFile } from "@tauri-apps/plugin-dialog";
  import {
    enumValues,
    saveRecord,
    tableEntries,
    tableRecord,
    type Entry,
    type Section,
    type TableRecord,
    type Value,
    type Column,
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

  // New entries: moves and items a mod adds after the game's own
  // (core/src/content.rs). `added` is null for other tables.
  const ADDS: Record<string, string> = { moves: "move", items: "item" };
  const noun = $derived(ADDS[tables[0]] ?? null);
  let added = $state<{ last: number; ids: number[]; ceiling: number } | null>(null);
  let dialog = $state<{ mode: "add" | "remove" } | null>(null);
  $effect(() => {
    void version;
    if (!noun) return;
    untrack(() => {
      invoke<{ last: number; ids: number[]; ceiling: number }>("content_added", { table: tables[0] })
        .then((a) => (added = a))
        .catch(() => (added = null));
    });
  });
  const isAdded = (index: number) => !!added && index > added.last;

  // The list again (names and count change with an add or a remove).
  async function refreshEntries() {
    forget(`entries/${tables[0]}`);
    entries = await tableEntries(tables[0]);
  }

  async function addEntry(from: number, name: string) {
    const id = await invoke<number>("content_add", { table: tables[0], from, name });
    play("save");
    reloadIcons();
    onSaved();
    await refreshEntries();
    drafts.delete(id);
    current = null;
    await open(id, false);
    list?.reveal(entries.findIndex((e) => e.index === id));
    toast(`Added ${name} as ${noun} ${id}`);
  }

  // New Pokemon (core species_add.rs): ids 715 and up.
  let speciesNew = $state<number[]>([]);
  let speciesDialog = $state<"add" | "remove" | null>(null);
  $effect(() => {
    void version;
    if (tables[0] !== "species") return;
    untrack(() => {
      invoke<number[]>("species_added")
        .then((s) => (speciesNew = s))
        .catch(() => (speciesNew = []));
    });
  });
  async function addSpecies(from: number, name: string) {
    const id = await invoke<number>("species_add", { from, name });
    play("save");
    reloadIcons();
    onSaved();
    await refreshEntries();
    current = null;
    await open(id, false);
    list?.reveal(entries.findIndex((e) => e.index === id));
    toast(`Added ${name} as Pokemon ${id}`);
  }
  async function removeSpecies(id: number) {
    const name = nameOf(id);
    const r = await invoke<{ renumbered: [number, number][]; updated: string[] }>("species_remove", { id });
    play("save");
    reloadIcons();
    onSaved();
    await refreshEntries();
    current = null;
    const next = entries.find((e) => e.index === id) ?? entries.find((e) => e.index === id - 1) ?? entries[1];
    if (next) await open(next.index, false);
    const parts = [`Removed ${name}`];
    if (r.renumbered.length) parts.push(`${r.renumbered.length} later Pokemon moved down one number`);
    if (r.updated.length) parts.push(`${r.updated.length} record${r.updated.length > 1 ? "s" : ""} updated`);
    toast(parts.join("; "));
  }

  // A trainer's party kind (custom moves bit 0, held items bit 1): saving
  // the trainer alone converts its party (core tables.rs convert_party).
  async function setPartyKind(kind: number) {
    if (current === null) return;
    busy = true;
    error = null;
    try {
      await saveRecord("trainers", current, { values: { partyType: kind }, lists: {}, season: 0 });
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

  // An added item's own picture, from a PNG (core content::set_item_icon).
  async function newIcon() {
    if (current === null) return;
    const item = current;
    const path = await openFile({ multiple: false, filters: [{ name: "PNG picture", extensions: ["png"] }], title: "Icon for this item" });
    if (typeof path !== "string") return;
    busy = true;
    try {
      await invoke("content_item_icon", { item, path });
      play("save");
      reloadIcons();
      onSaved();
      toast("Icon set");
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  async function removeEntry(id: number) {
    const name = nameOf(id);
    const r = await invoke<{ renumbered: [number, number][]; cleared: string[]; updated: string[] }>("content_remove", {
      table: tables[0],
      id,
    });
    play("save");
    reloadIcons();
    for (const [old] of r.renumbered) drafts.delete(old);
    drafts.delete(id);
    onSaved();
    await refreshEntries();
    current = null;
    const next = entries.find((e) => e.index === id) ?? entries.find((e) => e.index === id - 1) ?? entries[0];
    if (next) await open(next.index, false);
    const parts = [`Removed ${name}`];
    if (r.renumbered.length) parts.push(`${r.renumbered.length} later ${noun}${r.renumbered.length > 1 ? "s" : ""} moved down one number`);
    if (r.updated.length) parts.push(`${r.updated.length} record${r.updated.length > 1 ? "s" : ""} updated`);
    if (r.cleared.length) parts.push(`no longer used in: ${r.cleared.join(", ")}`);
    toast(parts.join("; "));
  }

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

  // The experience curve shown under a Pokemon: its growth rate's record.
  let expIndex = $state<number | null>(null);
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
    // A Pokemon's experience curve is the curve of its growth rate, not the
    // record with its own number.
    const ei = tables.indexOf("expCurves");
    if (ei > 0) {
      const g = rs[0]?.sections.flatMap((s) => (s.kind === "fields" ? s.values : [])).find((v) => v.key === "growthRate");
      const rate = g ? Number(g.value) : null;
      expIndex = rate;
      rs[ei] = rate === null ? null : await tableRecord("expCurves", rate, season).catch(() => null);
    }
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

  // One Save and Discard per entry: the records and the panels inside
  // (Name and text, In battle) together (lib/parts.svelte.ts).
  const entryParts = provideParts();

  // A trainer's per-member extras (eidos trainer_mons.txt), slot by slot,
  // saved with the entry's Save.
  let memberExtras = $state<Extras[]>([]);
  let extrasOrig = $state("[]");
  let openExtras = $state(new Set<number>());
  let natureNames = $state<string[]>([]);
  const extrasEmpty = (e: Extras) => Object.values(e).every((v) => v === null || v === undefined);
  const blankExtras = (): Extras => ({ ivs: null, evs: null, nature: null, shiny: null, friendship: null, ability: null });
  $effect(() => {
    const at = current;
    void version;
    if (tables[0] !== "trainers" || at === null) return;
    openExtras = new Set();
    invoke<Extras[]>("trainer_mons", { trainer: at })
      .then((m) => {
        if (current !== at) return;
        memberExtras = m.map((e) => ({ ...blankExtras(), ...e }));
        extrasOrig = JSON.stringify(memberExtras);
      })
      .catch(() => {});
    if (!natureNames.length)
      enumValues("nature")
        .then((n) => (natureNames = n))
        .catch(() => {});
  });
  $effect(() => {
    if (tables[0] !== "trainers" || readOnly) return;
    return entryParts.register("trainerMons", {
      dirty: () => JSON.stringify(memberExtras) !== extrasOrig,
      save: async () => {
        if (current === null) return;
        await invoke("set_trainer_mons", { trainer: current, members: memberExtras });
        extrasOrig = JSON.stringify(memberExtras);
      },
      discard: () => (memberExtras = JSON.parse(extrasOrig)),
    });
  });
  function toggleExtras(rn: number) {
    const next = new Set(openExtras);
    if (next.has(rn)) next.delete(rn);
    else next.add(rn);
    openExtras = next;
  }
  // Removing a member moves the later members' extras up with them.
  function removeMember(s: Extract<Section, { kind: "list" }>, rn: number) {
    s.rows.splice(rn, 1);
    memberExtras = [...memberExtras.slice(0, rn), ...memberExtras.slice(rn + 1), blankExtras()];
    openExtras = new Set([...openExtras].filter((i) => i !== rn).map((i) => (i > rn ? i - 1 : i)));
  }
  const recordsDirty = $derived(JSON.stringify(records) !== original);
  const dirty = $derived(recordsDirty || entryParts.dirty);
  // One section at a time (a species' stats, then its learnset...).
  let secTab = $state(0);
  const allSections = $derived(records.flatMap((r) => r?.sections ?? []));
  const activeTab = $derived(secTab < 0 ? secTab : Math.min(secTab, Math.max(0, allSections.length - 1)));
  // A Pokemon's tabs add two views of the Stats record that need not show
  // with the stats: its name and Pokedex text, and its TMs and tutors.
  const TEXT_TAB = -1;
  const MACHINE_TAB = -2;
  const speciesTabs = $derived(tables[0] === "species" && only === undefined && !single);
  const tabList = $derived.by((): [number, string][] => {
    const list: [number, string][] = allSections.map((s, i) => [i, sectionLabel(s)]);
    if (!speciesTabs) return list;
    const at = list.findIndex(([, l]) => l.startsWith("Level-up"));
    list.splice(at < 0 ? list.length : at + 1, 0, [MACHINE_TAB, "TMs and tutors"]);
    list.splice(1, 0, [TEXT_TAB, "Name and Pokedex"]);
    return list;
  });
  // Moves and items: the editor's parts as tabs (data, battle code, uses).
  type Panel = "data" | "text" | "medicine" | "battle" | "users";
  // An item's medicine block (healing, stat boosts, EVs, friendship): read
  // by the game only when "Has a medicine block" is 1, so it is a tab of its
  // own, shown only then. Its groups are known by their first field.
  const MEDICINE_FIRST = ["hpRestore", "atkStage", "hpEv", "friendshipDeltaLow"];
  const isMedicine = (vals: Value[]) => tables[0] === "items" && MEDICINE_FIRST.includes(vals[0]?.key ?? "");
  const hasMedicine = $derived(tables[0] === "items" && Number(fieldValue("extType") ?? 0) === 1);
  const PANELS = $derived.by((): [Panel, string][] => {
    const list: [Panel, string][] = [["data", "Data"], ["text", "Name and text"]];
    if (hasMedicine) list.push(["medicine", "Medicine"]);
    list.push(["battle", "In battle"], ["users", tables[0] === "moves" ? "Used by" : "Locations"]);
    return list;
  });
  let panel = $state<Panel>("data");
  const panelTabs = $derived(!!noun && only === undefined && !single && current !== null && current > 0);
  $effect(() => {
    if (!panelTabs || (panel === "medicine" && !hasMedicine)) panel = "data";
  });
  // Wild encounters show every method at once, as cards side by side.
  const cards = $derived(tables[0] === "encounters" && only === undefined && !single);
  const tabbed = $derived(only === undefined && allSections.length > 1 && !cards);
  // The chance of each wild slot (FIELD_ENCOUNT_PickGroundSlot,
  // PickWaterSlot, PickFishingSlot in ov021 field_encount.c; which method
  // uses which: FIELD_ENCOUNT_LoadSlots).
  const GROUND = [20, 20, 10, 10, 10, 10, 5, 5, 4, 4, 1, 1];
  const WATER = [60, 30, 5, 4, 1];
  const FISHING = [40, 40, 15, 4, 1];
  const SLOT_CHANCES: Record<string, number[]> = {
    grass: GROUND,
    doubleGrass: GROUND,
    shakingGrass: GROUND,
    surf: WATER,
    surfRipple: WATER,
    fishing: FISHING,
    fishingRipple: FISHING,
  };
  // Methods a place does not use (no Pokemon in any slot) stay hidden until
  // asked for: Driftveil City has no grass, only water.
  let showEmpty = $state(false);
  $effect(() => {
    void current;
    showEmpty = false;
  });
  const emptyList = (s: Section) => s.kind === "list" && s.rows.every((r) => !r[0]);
  const hiddenLists = $derived(cards && !showEmpty ? allSections.filter(emptyList) : []);
  const chances = (s: Section) => (tables[0] === "encounters" && s.kind === "list" ? SLOT_CHANCES[s.key] : undefined);
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
      await entryParts.save();
      for (let i = 0; i < tables.length && recordsDirty; i++) {
        const r = records[i];
        if (!r) continue;
        const values: Record<string, number> = {};
        const lists: Record<string, number[][]> = {};
        for (const s of r.sections) {
          if (s.kind === "fields") s.values.forEach((v) => (values[v.key] = Number(v.value)));
          else lists[s.key] = s.rows.map((row) => row.map(Number));
        }
        // The experience curve is saved under its growth rate (see reload).
        const at = tables[i] === "expCurves" && i > 0 ? expIndex : current;
        if (at === null) continue;
        await saveRecord(tables[i], at, { values, lists, season });
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

  // An evolution's parameter is an item, a move or a Pokemon by its method
  // (core tables.rs evolution_param_enum; enum evolutionMethod).
  const EVO_ITEM = [6, 8, 17, 18, 19, 20];
  function columnEnum(s: Extract<Section, { kind: "list" }>, c: { name: string; enum: string | null }, row: number[]): string | null {
    if (c.name !== "param" || !s.columns.some((x) => x.name === "method")) return c.enum;
    const method = Number(row[s.columns.findIndex((x) => x.name === "method")]);
    if (EVO_ITEM.includes(method)) return "item";
    if (method === 21) return "move";
    if (method === 22) return "species";
    return null;
  }
  // Names for those pickers load with the record's own enums.
  $effect(() => {
    if (tables.includes("evolutions")) for (const n of ["item", "move", "species"]) if (!names[n]) enumValues(n).then((v) => (names[n] = v));
  });

  // Fields under their headings, in schema order (a group's fields are next
  // to each other there).
  function groupsOf(values: Value[]): { label: string; values: Value[] }[] {
    const out: { label: string; values: Value[] }[] = [];
    for (const v of values) {
      const label = v.group ?? "";
      const last = out[out.length - 1];
      if (last && last.label === label) last.values.push(v);
      else out.push({ label, values: [v] });
    }
    return out;
  }
  // A box as wide as what it holds: a number, a short list, a searchable
  // list.
  // Dragging a base stat bar: the pointer's place along the bar sets the
  // stat (1 to 255).
  let statDrag = $state<Value | null>(null);
  function statSet(v: Value, e: PointerEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const f = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    v.value = Math.max(1, Math.round(f * 255));
  }
  function statDown(v: Value, e: PointerEvent) {
    statDrag = v;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    statSet(v, e);
  }
  function statMove(v: Value, e: PointerEvent) {
    if (statDrag === v) statSet(v, e);
  }
  function statUp() {
    statDrag = null;
  }
  function statKey(v: Value, e: KeyboardEvent) {
    const step = e.shiftKey ? 10 : 1;
    const n = Number(v.value);
    if (e.key === "ArrowRight" || e.key === "ArrowUp") v.value = Math.min(255, n + step);
    else if (e.key === "ArrowLeft" || e.key === "ArrowDown") v.value = Math.max(1, n - step);
    else return;
    e.preventDefault();
  }
  // A base stat's bar colour, low to high (the bands players use: under 50
  // weak, 100 and up strong).
  function statColor(n: number): string {
    if (n < 50) return "#d9534f";
    if (n < 80) return "#e8903a";
    if (n < 100) return "#e3c63d";
    if (n < 130) return "#5cb85c";
    return "#3fa7d6";
  }
  // How a list is picked: the search dropdown (long lists, and any list with
  // game icons, such as types, so the icon shows beside the name), tabs (two
  // to four choices whose names fit on one short row: gender, category) or a
  // plain dropdown. A value the list does not name falls back to the
  // dropdown, which shows it.
  function pickKind(en: string, value?: number): "search" | "popup" | "seg" | "select" {
    const list = names[en] ?? [];
    if (value !== undefined && value >= list.length) return "select";
    if (enumIcon(en) && list.length <= LONG) return "popup";
    if (list.length > LONG || enumIcon(en)) return "search";
    const chars = list.reduce((t, n) => t + n.length, 0);
    if (list.length >= 2 && list.length <= 4 && chars <= 22) return "seg";
    return "select";
  }
  function fieldSize(v: Value): string {
    const list = v.enum ? names[v.enum] : undefined;
    if (list?.length && v.enum) {
      const kind = pickKind(v.enum, v.value);
      if (kind === "seg") return "fit";
      if (kind === "popup") return "mid";
      if (list.length > LONG) return "wide";
      // A short list as wide as its longest choice.
      const longest = Math.max(...list.map((n) => n.length));
      return longest > 26 ? "xwide" : longest > 16 ? "wide" : "mid";
    }
    return "num";
  }
  // Evolutions: seven fixed rows, mostly empty. Show the rows in use and one
  // empty row to add with; the other empty rows stay hidden.
  function spareRow(s: Extract<Section, { kind: "list" }>, rn: number): boolean {
    if (!s.fixed || !s.columns.some((c) => c.name === "method")) return false;
    const empty = (r: number[]) => r.every((v) => !Number(v));
    return empty(s.rows[rn]) && s.rows.slice(0, rn).some(empty);
  }
  // A card's columns: who it is first (species, level, form, item), then the
  // rest in schema order.
  const CARD_FIRST = ["species", "level", "form", "item", "heldItem"];
  function cardOrder(s: Extract<Section, { kind: "list" }>): number[] {
    const idx = s.columns.map((_, i) => i);
    const rank = (i: number) => {
      const r = CARD_FIRST.indexOf(s.columns[i].name);
      return r < 0 ? CARD_FIRST.length : r;
    };
    return idx.sort((a, b) => rank(a) - rank(b) || a - b);
  }
  // A group too big for one card column: many checkboxes, or rows of
  // related fields (a move's three stat changes).
  function wideGroup(vals: Value[]): boolean {
    const flags = vals.filter((v) => !(v.enum && names[v.enum]?.length) && v.max === 1 && v.min === 0).length;
    return flags > 16 || vals.some((v) => v.rowStart);
  }
  // A note every field of a group shares ("0 to 31" on IVs): shown once,
  // on the group's heading, not on each field.
  function sharedNote(vals: Value[]): string | null {
    const first = vals[0]?.note;
    return vals.length > 1 && first && vals.every((v) => v.note === first) ? first : null;
  }
  // A table's columns in reading order: a learnset reads level, then move.
  function tableOrder(s: Extract<Section, { kind: "list" }>): number[] {
    const idx = s.columns.map((_, i) => i);
    const lv = s.columns.findIndex((c) => c.name === "level");
    return lv > 0 && s.columns.some((c) => c.name === "move") ? [lv, ...idx.filter((i) => i !== lv)] : idx;
  }
  // The party list's columns: each sized to what it holds, the remove
  // button last.
  function partyColumns(s: Extract<Section, { kind: "list" }>, who: number[]): string {
    const width = (ci: number) => {
      const c = s.columns[ci];
      const kind = c.enum && names[c.enum]?.length ? pickKind(c.enum) : null;
      // A fixed width, so the header lines up with every member's row.
      if (kind === "seg") {
        const list = names[c.enum!] ?? [];
        const chars = list.reduce((t, n) => t + n.length, 0);
        return `${Math.ceil(chars * 0.55 + list.length * 1.4)}rem`;
      }
      if (kind === "search" || kind === "popup") return "minmax(9rem, 1.3fr)";
      if (kind === "select") return "minmax(7rem, 1fr)";
      return "4.6rem";
    };
    return [...who.map(width), "auto", "1.8rem"].join(" ");
  }
  function columnSize(s: Extract<Section, { kind: "list" }>, c: Column, row: number[]): string {
    const en = columnEnum(s, c, row);
    return fieldSize({ enum: en } as Value);
  }
  // A list tab counts the rows in use (evolutions are seven fixed rows,
  // mostly empty).
  const sectionLabel = (s: Section) =>
    s.kind === "list" ? `${s.label} (${s.fixed ? s.rows.filter((r) => r.some((v) => Number(v))).length : s.rows.length})` : s.label;
</script>

{#snippet pick(obj: any, key: string | number, en: string | null, min: number, max: number, ro: boolean, name: string)}
  <span class="ctl">
  {#if en && names[en]?.length}
    {@const kind = pickKind(en, Number(obj[key]))}
    {#if kind === "search"}
      <SearchSelect bind:value={obj[key]} options={names[en]} icon={enumIcon(en)} disabled={ro} label={name} />
    {:else if kind === "popup"}
      <!-- A short list with game icons (types): every choice in a popup. -->
      <IconPicker bind:value={obj[key]} options={names[en]} icon={enumIcon(en)!} disabled={ro} label={name} />
    {:else if kind === "seg"}
      <!-- Two to four short choices (gender, category): tabs, one click. -->
      <div class="seg" role="radiogroup" aria-label={name}>
        {#each names[en] as n, i}
          <button type="button" role="radio" aria-checked={Number(obj[key]) === i} class:on={Number(obj[key]) === i} disabled={ro} onclick={() => (obj[key] = i)}>
            {n || `(${i})`}
          </button>
        {/each}
      </div>
    {:else}
      <select bind:value={obj[key]} disabled={ro} aria-label={name}>
        {#each names[en] as n, i}<option value={i}>{n || `(${i})`}</option>{/each}
        {#if obj[key] >= names[en].length}<option value={obj[key]}>({obj[key]})</option>{/if}
      </select>
    {/if}
  {:else}
    {#if obj.scale}
      <!-- Stored divided by the scale (a price / 10): shown and typed whole. -->
      <input
        type="number"
        min={min * obj.scale}
        max={max * obj.scale}
        step={obj.scale}
        value={Number(obj[key]) * obj.scale}
        onchange={(e) => (obj[key] = Math.min(max, Math.max(min, Math.round(Number(e.currentTarget.value) / obj.scale))))}
        readonly={ro}
        aria-label={name}
      />
    {:else}
      <input type="number" {min} {max} bind:value={obj[key]} onchange={() => fix(obj, key, min, max)} readonly={ro} aria-label={name} />
    {/if}
  {/if}
  </span>
{/snippet}

<div class="tables" class:single role="presentation" onkeydown={keys}>
  {#if !single}
  <aside class="entries" onkeydown={step} role="presentation">
    <div class="list-head">
      <input class="search" placeholder="Search {entries.length} entries" aria-label="Search entries" bind:value={filter} onkeydown={(e) => e.key === "Escape" && (filter = "")} />
      {#if !readOnly && !single && tables[0] === "species"}
        <button class="new" disabled={busy} onclick={() => ((speciesDialog = "add"), play("decide"))} title="Duplicate the selected Pokemon">New</button>
      {:else if !readOnly && !single && noun && added}
        <button class="new" disabled={busy} onclick={() => ((dialog = { mode: "add" }), play("decide"))} title="Duplicate the selected {noun}">New</button>
      {/if}
    </div>
    <VirtualList bind:this={list} items={shown} height={34} key={(e) => e.index} label="Entries">
      {#snippet row(e)}
        <button class="row-item" aria-current={e.index === current} onclick={() => open(e.index)} oncontextmenu={(ev) => entryMenu(ev, e.index)}>
          <span class="num muted">{e.index}</span>
          {#if hasIcons}
            {@const src = rowIcon(e.index)}
            {#if src}<img class="px ico" {src} alt="" loading="lazy" />{:else}<span class="ico"></span>{/if}
          {/if}
          <span class="label">{e.name}</span>
          {#if isAdded(e.index)}<span class="tag new" title="Added by this mod">new</span>{/if}
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
    <div class="head entry-head">
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
        {#if tables[0] === "species" && !single}
          {#if current !== null && speciesNew.includes(current)}
            <button class="act" disabled={busy} onclick={() => ((speciesDialog = "remove"), play("decide"))}>Remove Pokemon</button>
          {/if}
        {/if}
        {#if noun && added && !single}
          {#if current !== null && isAdded(current)}
            {#if tables[0] === "items"}
              <button class="act" disabled={busy} onclick={newIcon} title="A 32 x 32 PNG; its colours become the icon's palette">New icon...</button>
            {/if}
            <button class="act" disabled={busy} onclick={() => ((dialog = { mode: "remove" }), play("decide"))}>Remove {noun}</button>
          {/if}
        {/if}
        <button class="act" disabled={!dirty || busy} onclick={() => (entryParts.discard(), reload())}>Discard</button>
        <button class="act primary" disabled={!dirty || busy} onclick={save}>Save</button>
      {/if}
    </div>
    {#if error}<p class="bad pad">{error}</p>{/if}

    {#if panelTabs}
      <!-- Moves and items: the data, what it runs in battle, and where it is
           used, as tabs; each fits the window on its own. -->
      <div class="sec-tabs" role="tablist">
        {#each PANELS as [p, label]}
          <button role="tab" aria-selected={panel === p} class:primary={panel === p} onclick={() => (panel = p)}>
            {label}
          </button>
        {/each}
      </div>
    {/if}
    {#if tabbed}
      <div class="sec-tabs" role="tablist">
        {#each tabList as [i, label]}
          <button role="tab" aria-selected={activeTab === i} class:primary={activeTab === i} onclick={() => (secTab = i)}>{label}</button>
        {/each}
      </div>
    {/if}
    <div class="scroll body" class:cards>
      {#if tables[0] === "trainers" && current !== null && tabbed && allSections[activeTab]?.kind === "list" && !readOnly}
        {@const kind = Number(fieldValue("partyType") ?? 0)}
        <div class="window block party-kind">
          <label class="flag">
            <input type="checkbox" checked={(kind & 1) === 1} disabled={busy || dirty} onchange={(e) => setPartyKind((kind & ~1) | ((e.currentTarget as HTMLInputElement).checked ? 1 : 0))} />
            Custom moves
          </label>
          <label class="flag">
            <input type="checkbox" checked={(kind & 2) === 2} disabled={busy || dirty} onchange={(e) => setPartyKind((kind & ~2) | ((e.currentTarget as HTMLInputElement).checked ? 2 : 0))} />
            Held items
          </label>
          <span class="muted">
            {dirty ? "Save or discard your edits first." : "Custom moves prefill with each Pokemon's level-up moveset."}
          </span>
        </div>
      {/if}
      {#if !records.length && loading}
        <p class="muted">Loading...</p>
      {/if}
      {#if ["moves", "items", "species", "trainers"].includes(tables[0]) && current !== null && current > 0 && only === undefined && !single}
        <!-- Kept while hidden: unsaved text survives a tab switch. -->
        <div hidden={speciesTabs ? activeTab !== TEXT_TAB : panelTabs ? panel !== "text" : tabbed && activeTab !== 0}>
          <TextPanel kind={tables[0] as "moves" | "items" | "species" | "trainers"} id={current} {readOnly} onSaved={() => (onSaved(), refreshEntries())} />
        </div>
      {/if}
      {#if noun && current !== null && records[0] && only === undefined && !single && panel === "data"}
        <EffectSummary
          table={tables[0] as "moves" | "items"}
          id={current}
          record={records[0]}
          {names}
          added={isAdded(current)}
          onOpen={(other) => {
            open(other);
            list?.reveal(entries.findIndex((e) => e.index === other));
          }}
        />
      {/if}
      {#if panelTabs && added && current !== null && current > 0}
        <!-- Kept while hidden: unsaved edits survive a tab switch. -->
        <div hidden={panel !== "battle"}>
          <BattleHandler table={tables[0] as "moves" | "items"} id={current} last={added.last} {version} {readOnly} {onSaved} />
          {#if tables[0] === "moves"}
            <MoveAnim id={current} {version} {readOnly} {onSaved} />
          {/if}
        </div>
      {/if}
      {#if speciesTabs && activeTab === MACHINE_TAB}
        {@const fs = records[0]?.sections.find((x) => x.kind === "fields")}
        {#if fs && fs.kind === "fields"}
          <div class="window block"><MachineGrid values={fs.values} {readOnly} /></div>
        {/if}
      {/if}
      {#if panel === "data" || panel === "medicine"}
      {#each records as r, ri}
        {#each r?.sections ?? [] as s, si}
          {#if (only !== undefined ? sectionIndex(ri, si) === only : !tabbed || sectionIndex(ri, si) === activeTab) && !hiddenLists.includes(s)}
          <div class="window block" class:card={cards && s.kind === "list"}>
            <!-- A fields section with headings needs no title: it would repeat the screen's name. -->
            {#if !single && !tabbed && tables[ri] !== "expCurves" && !(s.kind === "fields" && s.values.some((v) => v.group))}<h3>{s.label}</h3>{/if}
            {#if s.kind === "fields"}
              {@const isFlag = (v: (typeof s.values)[number]) => !(v.enum && names[v.enum]?.length) && v.max === 1 && v.min === 0}
              {@const isMachine = (v: (typeof s.values)[number]) => tables[ri] === "species" && MACHINE_KEYS.includes(v.key)}
              {#snippet fieldset(vals: typeof s.values)}
                {#if vals.some((v) => !isFlag(v))}
                  <div class="grid">
                    {#each vals.filter((v) => !isFlag(v)) as v}
                      {#if v.rowStart}<span class="break"></span>{/if}
                      <!-- Tabs and icon pickers hold buttons: a label around them would click the first one. -->
                      <svelte:element this={v.enum && pickKind(v.enum, v.value) === "seg" ? "div" : "label"} class="field {fieldSize(v)}">
                        <span class="name">
                          {v.label}
                          {#if v.note && v.note !== sharedNote(vals)}<span class="info" use:tip={v.note}>i</span>{/if}
                          {#if v.inferred}<span class="tag" title="Meaning inferred, not confirmed in the game's code">?</span>{/if}
                        </span>
                        {@render pick(v, "value", v.enum, v.min, v.max, readOnly || v.readonly || (tables[ri] === "trainers" && v.key === "partyType"), v.label)}
                      </svelte:element>
                    {/each}
                  </div>
                {/if}
                {#if vals.some(isFlag)}
                  <div class="flags" class:alone={!vals.some((v) => !isFlag(v))}>
                    {#each vals.filter(isFlag) as v}
                      <label class="flag" title={v.note ?? ""}>
                        <input type="checkbox" checked={v.value === 1} disabled={readOnly || v.readonly} onchange={(e) => (v.value = (e.currentTarget as HTMLInputElement).checked ? 1 : 0)} />
                        <span>{v.label}</span>
                        {#if v.inferred}<span class="tag" title="Meaning inferred, not confirmed in the game's code">?</span>{/if}
                      </label>
                    {/each}
                  </div>
                {/if}
              {/snippet}
              {#if tables[ri] === "expCurves"}
                {@const rateAt = ri > 0 ? expIndex : current}
                <ExpCurve values={s.values} {readOnly} rate={rateAt !== null ? (names.growthRate?.[rateAt] ?? null) : null} />
              {:else}
              {@const gs = groupsOf(s.values.filter((v) => !isMachine(v))).filter((g) => isMedicine(g.values) === (panel === "medicine"))}
              <!-- Headed groups are cards in a grid, each field one row (label,
                   then a box sized to its value); a large group takes the full
                   width. Without headings, the fields wrap as before. -->
              <div class="groups" class:sheet={gs.some((g) => g.label)}>
              {#each gs as g}
                {#if g.label === "Advanced"}
                  <details class="group adv">
                    <summary>Advanced</summary>
                    {@render fieldset(g.values)}
                  </details>
                {:else if g.values.length && g.values.every((v) => /^rank(Type|Value|Percent)\d$/.test(v.key))}
                  <!-- A move's stat changes: one row each (stat, stages, chance). -->
                  {@const rows = [...new Set(g.values.map((v) => v.key.slice(-1)))]}
                  <div class="group">
                    <h4>{g.label}</h4>
                    <table class="mini">
                      <thead><tr><th>Stat</th><th>Stages</th><th>Chance (%)</th></tr></thead>
                      <tbody>
                        {#each rows as n}
                          <tr>
                            {#each ["rankType", "rankValue", "rankPercent"] as k}
                              {@const v = g.values.find((x) => x.key === `${k}${n}`)}
                              <td>{#if v}{@render pick(v, "value", v.enum, v.min, v.max, readOnly || v.readonly, `${v.label}`)}{/if}</td>
                            {/each}
                          </tr>
                        {/each}
                      </tbody>
                    </table>
                  </div>
                {:else if tables[ri] === "species" && g.values.length === 6 && g.values.every((v) => v.key.startsWith("base"))}
                  <!-- Base stats: a bar per stat on the 255 scale and the total. -->
                  {@const total = g.values.reduce((t, v) => t + Number(v.value || 0), 0)}
                  <div class="group">
                    <h4>{g.label} <span class="total">Total: {total}</span></h4>
                    <div class="stats">
                      {#each g.values as v}
                        <div class="stat">
                          <span class="sname">{v.label}</span>
                          <input type="number" min={v.min} max={v.max} bind:value={v.value} readonly={readOnly || v.readonly} aria-label={v.label} />
                          <!-- Drag or click along the bar to set the stat; arrow keys step it. -->
                          <span
                            class="track"
                            class:live={!(readOnly || v.readonly)}
                            role="slider"
                            tabindex={readOnly || v.readonly ? -1 : 0}
                            aria-label={v.label}
                            aria-valuemin={1}
                            aria-valuemax={255}
                            aria-valuenow={Number(v.value)}
                            onpointerdown={(e) => !(readOnly || v.readonly) && statDown(v, e)}
                            onpointermove={(e) => statMove(v, e)}
                            onpointerup={statUp}
                            onpointercancel={statUp}
                            onkeydown={(e) => !(readOnly || v.readonly) && statKey(v, e)}
                          ><span class="bar" style:width="{Math.min(100, (Number(v.value) / 255) * 100)}%" style:background={statColor(Number(v.value))}></span></span>
                        </div>
                      {/each}
                    </div>
                  </div>
                {:else}
                  <div class="group" class:span={wideGroup(g.values) || !g.label}>
                    {#if g.label}<h4>{g.label}{#if sharedNote(g.values)} <span class="info" use:tip={sharedNote(g.values) ?? ""}>i</span>{/if}</h4>{/if}
                    {@render fieldset(g.values)}
                  </div>
                {/if}
              {/each}
              </div>
              {/if}
              {#if s.values.some(isMachine) && !speciesTabs}
                <MachineGrid values={s.values} readOnly={readOnly} />
              {/if}
            {:else}
              {#if s.rows.length === 0}<p class="muted">Nothing here yet.</p>{/if}
              {#if !single && s.columns.length > 6 && s.rows.length}
                <!-- A trainer's party: one header, then two lines per member
                     (who it is, then its moves), so six members fit. -->
                {@const order = cardOrder(s)}
                {@const moves = order.filter((ci) => /^move\d$/.test(s.columns[ci].name))}
                {@const who = order.filter((ci) => !moves.includes(ci))}
                <div class="party">
                  <div class="pline phead" style:grid-template-columns={partyColumns(s, who)}>
                    {#each who as ci}<span>{s.columns[ci].label}</span>{/each}
                    <span></span>
                    <span></span>
                  </div>
                  {#each s.rows as row, rn}
                    {@const diff = s.columns.findIndex((c) => c.name === "difficulty")}
                    <div class="member">
                      <div class="pline" style:grid-template-columns={partyColumns(s, who)}>
                        {#each who as ci}
                          {@const c = s.columns[ci]}
                          <div class="pcell">{@render pick(row, ci, columnEnum(s, c, row), c.min, c.max, readOnly, c.label)}</div>
                        {/each}
                        <!-- IVs, EVs, nature, shiny, friendship, ability (eidos trainer_mons). -->
                        <button
                          class="quiet more"
                          class:set={memberExtras[rn] && !extrasEmpty(memberExtras[rn])}
                          aria-expanded={openExtras.has(rn)}
                          onclick={() => toggleExtras(rn)}
                          title="IVs, EVs, nature, shiny, friendship, ability"
                        >Extras</button>
                        {#if !s.fixed && !readOnly}
                          <button class="quiet x" onclick={() => removeMember(s, rn)} aria-label="Remove member {rn + 1}" title="Remove">×</button>
                        {:else}<span></span>{/if}
                      </div>
                      {#if moves.length}
                        <div class="pline pmoves">
                          {#each moves as ci}
                            {@const c = s.columns[ci]}
                            <div class="pcell">{@render pick(row, ci, columnEnum(s, c, row), c.min, c.max, readOnly, c.label)}</div>
                          {/each}
                        </div>
                      {/if}
                      {#if openExtras.has(rn) && memberExtras[rn]}
                        <MemberExtras
                          bind:extras={memberExtras[rn]}
                          gameIv={Math.floor(((diff >= 0 ? Number(row[diff]) : 0) * 31) / 255)}
                          natures={natureNames}
                          {readOnly}
                        />
                      {/if}
                    </div>
                  {/each}
                </div>
              {:else if single || s.columns.length > 6}
                <!-- Narrow, or many columns (a trainer's party): one card per
                     row, the most telling column first, each box sized to
                     what it holds. -->
                <div class="rows" class:wide-cards={!single}>
                  {#each s.rows as row, rn}
                    {@const sp = s.columns.findIndex((c) => c.name === "species")}
                    {@const lv = s.columns.findIndex((c) => c.name === "level")}
                    <div class="row-card">
                      <div class="row-head">
                        <strong class="who">
                          {rn + 1}
                          {#if sp >= 0 && Number(row[sp])}
                            {#if speciesIcon(Number(row[sp]))}<img class="px" src={speciesIcon(Number(row[sp]))} alt="" />{/if}
                            {names.species?.[Number(row[sp])] ?? ""}{#if lv >= 0}<span class="muted"> Lv {row[lv]}</span>{/if}
                          {/if}
                        </strong>
                        {#if !s.fixed && !readOnly}
                          <button class="quiet" onclick={() => s.rows.splice(rn, 1)} aria-label="Remove row {rn + 1}">Remove</button>
                        {/if}
                      </div>
                      <div class="grid">
                        {#each cardOrder(s) as ci}
                          {@const c = s.columns[ci]}
                          {#if c.name === "move1"}<span class="break"></span>{/if}
                          <label class="field {single ? '' : columnSize(s, c, row)}">
                            <span class="name">{c.label}</span>
                            {@render pick(row, ci, columnEnum(s, c, row), c.min, c.max, readOnly, c.label)}
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
                      {#if chances(s)}<th style="width: 4rem" title="How often this slot is picked">Chance</th>{/if}
                      {#each tableOrder(s) as ci}{@const c = s.columns[ci]}
                        <th style:width={(c.enum && names[c.enum]?.length) || (c.name === "param" && s.columns.some((x) => x.name === "method")) ? "16rem" : "4.6rem"}>{c.label}</th>
                      {/each}
                      {#if !s.fixed && !readOnly}<th style="width: 5.5rem"></th>{/if}
                    </tr>
                  </thead>
                  <tbody>
                    {#each s.rows as row, rn}
                      {#if !spareRow(s, rn)}
                      <tr>
                        {#if chances(s)}<td class="chance muted">{chances(s)?.[rn] ?? "?"}%</td>{/if}
                        {#each tableOrder(s) as ci}{@const c = s.columns[ci]}
                          <td>{@render pick(row, ci, columnEnum(s, c, row), c.min, c.max, readOnly, c.label)}</td>
                        {/each}
                        {#if !s.fixed && !readOnly}
                          <td><button class="quiet" onclick={() => s.rows.splice(rn, 1)} aria-label="Remove row {rn + 1}">Remove</button></td>
                        {/if}
                      </tr>
                      {/if}
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
      {/if}
      {#if hiddenLists.length}
        <div class="window block empty-methods">
          <span class="muted">No encounters: {hiddenLists.map((s) => s.label.replace(/ slots$/, "")).join(", ")}.</span>
          <button class="quiet" onclick={() => (showEmpty = true)}>Show</button>
        </div>
      {/if}
      {#if panelTabs && panel === "users" && tables[0] === "moves" && current !== null}
        <MoveUsers id={current} {version} />
      {/if}
      {#if panelTabs && panel === "users" && tables[0] === "items" && current !== null}
        <ItemUsers id={current} {version} />
      {/if}
    </div>
  </section>
</div>

{#if speciesDialog}
  <ContentDialog
    mode={speciesDialog}
    noun="Pokemon"
    names={(() => {
      const out: string[] = [];
      for (const e of entries) if (e.index <= 649 || speciesNew.includes(e.index)) out[e.index] = e.name;
      return Array.from(out, (n) => n ?? "");
    })()}
    from={current !== null && (current <= 649 || speciesNew.includes(current)) ? current : 1}
    removing={current !== null ? { id: current, name: nameOf(current) } : null}
    icon={rowIcon}
    onAdd={addSpecies}
    onRemove={removeSpecies}
    onClose={() => (speciesDialog = null)}
  />
{/if}

{#if dialog && noun}
  <ContentDialog
    mode={dialog.mode}
    {noun}
    names={entries.map((e) => e.name)}
    from={current ?? 1}
    removing={current !== null ? { id: current, name: nameOf(current) } : null}
    icon={rowIcon}
    onAdd={addEntry}
    onRemove={removeEntry}
    onClose={() => (dialog = null)}
  />
{/if}

<style>
  .block.party-kind {
    display: flex;
    flex-direction: row;
    justify-content: flex-start;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.5rem 1.2rem;
    padding: 0.7rem 1rem;
  }
  .party-kind .muted {
    font-size: 0.85rem;
  }
  .flags {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(14rem, 1fr));
    gap: 0.35rem 1rem;
    margin-top: 0.7rem;
  }
  .flags.alone {
    margin-top: 0;
  }
  .flag {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    cursor: pointer;
  }
  .flag input {
    margin: 0;
  }
  .tag.new {
    color: var(--accent-ink);
    background: var(--accent);
    border-color: var(--accent-dark);
  }
  .tables {
    display: grid;
    grid-template-columns: 20rem minmax(0, 1fr);
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
    gap: 0.4rem 0.6rem;
  }
  /* A list row in the narrow form: its columns share the width. */
  .row-card .field {
    flex: 1 1 8rem;
  }
  .single .field .name {
    font-size: 0.78rem;
  }
  .field select,
  .field input[type="number"] {
    min-width: 0;
    max-width: 100%;
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
  .who {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }
  .who img {
    width: 32px;
    height: 32px;
    margin: -8px 0;
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
  /* Wild encounters: the method lists as cards side by side; anything else
     takes the whole row. */
  .body.cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(33rem, 1fr));
    align-items: start;
  }
  .body.cards > :not(.card) {
    grid-column: 1 / -1;
  }
  .card h3 {
    margin: 0;
  }
  .empty-methods {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    grid-column: 1 / -1;
  }
  td.chance {
    font-variant-numeric: tabular-nums;
    text-align: right;
    padding-right: 0.6rem;
  }
  /* Fields wrap in a row, each as wide as what it holds. */
  .grid {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 0.6rem 1rem;
  }
  .grid .break {
    flex-basis: 100%;
    height: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    min-width: 0;
  }
  /* A number box grows with a long label instead of wrapping it. */
  .field.num {
    min-width: 8.5rem;
    max-width: 13rem;
  }
  .field.num .name {
    white-space: nowrap;
  }
  .field.mid {
    width: 13rem;
  }
  .field.wide {
    width: 16rem;
  }
  .field.xwide {
    width: 21rem;
  }
  .field.fit {
    width: auto;
  }
  /* A trainer's party: shared header, two lines per member. */
  .party {
    display: flex;
    flex-direction: column;
  }
  .pline {
    display: grid;
    align-items: center;
    gap: 0.5rem;
  }
  .phead {
    padding: 0 0 0.3rem;
    font-size: 0.8rem;
    color: var(--muted);
  }
  .member {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    padding: 0.45rem 0;
    border-top: 1px solid var(--rim);
  }
  .pmoves {
    grid-template-columns: repeat(4, minmax(0, 1fr)) 1.8rem;
    padding-left: 1.2rem;
  }
  .pcell {
    min-width: 0;
  }
  /* A member's Extras button; marked when it holds the mod's own numbers. */
  .more {
    padding: 0.2rem 0.55rem;
    font-size: 0.8rem;
  }
  .more.set {
    color: var(--accent);
    border-color: var(--accent);
  }
  .x {
    padding: 0.1rem 0.4rem;
    font-size: 1.1rem;
    line-height: 1;
  }
  /* A small table inside a card (a move's stat changes). */
  table.mini {
    width: 100%;
    table-layout: fixed;
    background: none;
    border: none;
  }
  table.mini th {
    background: none;
    text-align: left;
    font-weight: 400;
    font-size: 0.85rem;
    color: var(--muted);
  }
  table.mini th {
    padding: 0 0.3rem 0.25rem 0;
  }
  table.mini td {
    padding: 0.12rem 0.3rem 0.12rem 0;
  }
  table.mini th:first-child,
  table.mini td:first-child {
    width: 45%;
  }
  /* Base stats: name, value, bar. */
  .total {
    margin-left: 0.6rem;
    color: var(--ink);
    text-transform: none;
    letter-spacing: normal;
  }
  .stats {
    display: grid;
    gap: 0.3rem;
    max-width: 42rem;
  }
  .stat {
    display: grid;
    grid-template-columns: 5rem 5.5rem minmax(0, 1fr);
    align-items: center;
    gap: 0.75rem;
  }
  .stat .sname {
    color: var(--muted);
    font-size: 0.85rem;
  }
  .track {
    height: 0.7rem;
    border-radius: 999px;
    background: var(--panel-3);
    overflow: hidden;
    touch-action: none;
  }
  .track.live {
    cursor: ew-resize;
  }
  .track.live:hover,
  .track.live:focus-visible {
    outline: 2px solid var(--rim);
    outline-offset: 2px;
  }
  .track .bar {
    display: block;
    height: 100%;
    border-radius: 999px;
  }
  /* Every control sits in one box; whatever it is fills that box. */
  .ctl {
    display: block;
    min-width: 0;
  }
  .ctl > :global(*:not(.seg)) {
    width: 100%;
  }
  /* Tabs for two to four choices. */
  .seg {
    display: inline-flex;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    overflow: hidden;
    background: var(--bg);
  }
  .seg button {
    border: none;
    border-radius: 0;
    box-shadow: none;
    background: none;
    padding: 0.35rem 0.8rem;
    font-weight: 600;
    color: var(--muted);
  }
  .seg button + button {
    border-left: 1px solid var(--rim);
  }
  .seg button.on {
    background: var(--accent);
    color: var(--accent-ink);
  }
  /* Tabs keep their own width inside a card's shared row. */
  .row-card .field.fit {
    flex: none;
  }
  /* A field's explanation: a small marker that shows it on hover. */
  .info {
    position: relative;
    display: inline-grid;
    place-items: center;
    width: 1rem;
    height: 1rem;
    border-radius: 50%;
    border: 1px solid var(--muted);
    font-size: 0.65rem;
    font-weight: 700;
    font-style: normal;
    cursor: help;
    text-transform: none;
    letter-spacing: normal;
    vertical-align: middle;
  }
  .group h4 {
    margin: 0 0 0.45rem;
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: var(--muted);
  }
  /* Headed groups: cards in a grid, three across on a wide window. */
  /* Cards flow down columns (up to three, 18rem at least), each in the
     next free space, so a short card never leaves a hole under it; a
     full-width group spans every column. */
  .groups.sheet {
    columns: 18rem 3;
    column-gap: 1.6rem;
  }
  .groups.sheet > .group {
    min-width: 0;
    break-inside: avoid;
    margin-bottom: 0.9rem;
  }
  .groups.sheet > .span {
    column-span: all;
  }
  /* In a card, each field is one row: the label, then its box. */
  /* A card's fields share one label column, as wide as its longest label,
     and one box column (each field's label and box are cells of it). */
  .groups.sheet > .group:not(.span) .grid {
    display: grid;
    grid-template-columns: minmax(5rem, max-content) minmax(0, 1fr);
    align-items: center;
    gap: 0.3rem 0.75rem;
  }
  /* The label column, then the box beside it, sized to its value; a list
     with long choices takes the card's width under its label. */
  /* Every field in a card: the label column, then its box. A number box is
     narrow; a list fills the rest of the row. One layout for the whole card. */
  .groups.sheet > .group:not(.span) .field {
    display: contents;
  }
  .groups.sheet > .group:not(.span) .field .name {
    text-align: left;
    max-width: 10rem;
  }
  .groups.sheet > .group:not(.span) .field .name {
    min-width: 0;
    white-space: normal;
  }
  .groups.sheet > .group:not(.span) .field .ctl {
    width: 5.5rem;
  }
  .groups.sheet > .group:not(.span) .field.mid .ctl,
  .groups.sheet > .group:not(.span) .field.wide .ctl,
  .groups.sheet > .group:not(.span) .field.xwide .ctl {
    width: 100%;
  }
  .groups.sheet > .group:not(.span) .field.fit .ctl {
    width: auto;
  }
  /* Tabs beside a label: a little smaller, to fit a card's row. */
  .groups.sheet > .group:not(.span) .seg button {
    padding: 0.3rem 0.55rem;
    font-size: 0.85rem;
  }
  /* A divider above each card's heading separates the sections. */
  .groups.sheet > .group {
    padding-top: 0.7rem;
    border-top: 1px solid var(--rim);
  }
  .groups.sheet > .group:not(.span) .flags {
    grid-template-columns: 1fr;
    gap: 0.25rem;
  }
  .groups.sheet > .group:not(.span) .break {
    display: none;
  }
  /* Base stats in a card: tighter columns. */
  .groups.sheet .stat {
    grid-template-columns: 4rem 4.5rem minmax(0, 1fr);
    gap: 0.5rem;
  }
  .group.adv summary {
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: var(--muted);
    margin-bottom: 0.45rem;
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
  table {
    width: max-content;
    max-width: 100%;
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
    padding: 0.1rem 0.3rem;
  }
  /* Table rows a little tighter than a form's (a learnset of 20 fits). */
  td input,
  td select {
    width: 100%;
    padding-top: 0.2rem;
    padding-bottom: 0.2rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
