<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { asset, saveTextBank, sounds, textBank, type SoundInfo } from "../lib/backend";
  import { tableEntries, type Entry, type Value } from "../lib/tables";
  import { play } from "../lib/sfx";
  import { once } from "../lib/cache.svelte";
  import { FLAG_BLOCKED, kindName, tileCss, tileLabel, tileRgb } from "../lib/terrain";
  import TableEditor from "./TableEditor.svelte";
  import WildEditor from "./WildEditor.svelte";

  // One map: click things to edit them, drag to move them, paint tiles.
  let {
    zone,
    zones = [],
    readOnly,
    onBack,
    onSaved,
  }: {
    zone: number;
    zones?: { id: number; label: string }[];
    readOnly: boolean;
    onBack: () => void;
    onSaved: () => void;
  } = $props();

  type Kind = "npc" | "warp" | "pos" | "bg";
  interface MapEvent {
    kind: Kind;
    index: number;
    x: number;
    z: number;
    w: number;
    h: number;
    trainer: number | null;
    toZone: number | null;
    values: Value[];
  }
  interface ZoneInfo {
    id: number;
    place: string;
    encounters: number | null;
    music: number[];
  }
  interface ZoneMap {
    zone: ZoneInfo;
    width: number;
    height: number;
    terrain: number[];
    flags: number[];
    events: MapEvent[];
  }
  interface MessageRef {
    bank: number;
    id: number;
    text: string;
  }
  interface ScriptInfo {
    id: number;
    messages: MessageRef[];
    trainers: number[];
    items: { item: number; count: number }[];
    gifts: { species: number; level: number }[];
    flagsSet: number[];
    flagsCleared: number[];
    warps: { zone: number; x: number | null; z: number | null }[];
    music: number[];
    complete: boolean;
  }
  interface EventScript {
    kind: Kind;
    index: number;
    scriptId: number | null;
    script: ScriptInfo | null;
  }
  type Pt = { x: number; z: number };
  type Rect = { x: number; z: number; w: number; h: number };
  type Tool = "select" | "paint" | "box" | "fill" | "pick";
  type Add = "npc" | "bg" | "item" | "warp" | "pos" | "prop";
  // A placed model: building, tree, sign post... Positions in map tiles.
  interface Prop {
    chunk: number;
    index: number;
    x: number;
    z: number;
    y: number;
    rot: number;
    model: number;
  }
  type TileWrite = [number, number, number, number];
  type Step =
    | { t: "tiles"; before: TileWrite[] }
    | { t: "move"; kind: Kind; index: number; x: number; z: number }
    | { t: "add"; kind: Kind; index: number }
    | { t: "prop"; chunk: number; index: number; x: number; z: number; y: number; rot: number };

  const TOOLS: [Tool, string, string][] = [
    ["select", "Select", "V"],
    ["paint", "Paint", "B"],
    ["box", "Box", "R"],
    ["fill", "Fill", "G"],
    ["pick", "Pick", "I"],
  ];
  const ADDS: [Add, string][] = [
    ["npc", "Person"],
    ["bg", "Sign"],
    ["item", "Hidden item"],
    ["warp", "Warp"],
    ["pos", "Trigger"],
    ["prop", "Object"],
  ];
  const KIND_LABEL: Record<Kind, string> = { npc: "Person", warp: "Warp", pos: "Trigger", bg: "Sign" };
  const HINT: Record<Tool, string> = {
    select: "Click to select. Drag to move. Drag on empty ground to select tiles.",
    paint: "Click or drag to paint. Right click picks a tile.",
    box: "Drag a box to fill it.",
    fill: "Click to fill an area of the same tile.",
    pick: "Click a tile to paint with it.",
  };
  const SEASONS = ["Spring", "Summer", "Autumn", "Winter"];
  const DIRS = ["Up", "Down", "Left", "Right", "Any side", "Left or right", "Up or down"];

  // svelte-ignore state_referenced_locally
  let current = $state(zone);
  let map = $state<ZoneMap | null>(null);
  // Tile arrays stay plain for speed; `tick` redraws after a change.
  let terrain = $state.raw<number[]>([]);
  let flags = $state.raw<number[]>([]);
  let tick = $state(0);
  let tool = $state<Tool>("select");
  let adding = $state<Add | null>(null);
  let brush = $state({ terrain: 0, flags: 0 });
  let picked = $state<MapEvent | null>(null);
  let placed = $state<Prop[]>([]);
  let propModels = $state<number[]>([]);
  let pickedProp = $state<Prop | null>(null);
  let showProps = $state(false);
  // The model "Add object" places.
  let newModel = $state<number | null>(null);
  // Bumped when placed models change, so the picture is drawn again.
  let renderStamp = $state(0);
  // The side panel's tab, per what is selected.
  let ptab = $state("main");
  // Only a different selection resets it; a reload of the same one keeps it.
  const selKey = $derived(
    picked ? `e:${picked.kind}:${picked.index}` : pickedProp ? `p:${pickedProp.chunk}:${pickedProp.index}` : "",
  );
  let lastKey = "";
  $effect(() => {
    if (selKey !== lastKey) {
      lastKey = selKey;
      ptab = "main";
    }
  });
  let sel = $state<Rect | null>(null);
  let moving = $state<Pt | null>(null);
  let boxing = $state<Rect | null>(null);
  let hover = $state<Pt | null>(null);
  let hoverEvent = $state<MapEvent | null>(null);
  let mouse = $state({ x: 0, y: 0 });
  let scale = $state(12);
  let pan = $state({ x: 0, y: 0 });
  let grid = $state(true);
  let look = $state<"game" | "tiles">("game");
  let renderOk = $state(true);
  let preview = $state(false);
  let undo = $state<Step[]>([]);
  let edits = $state<Record<string, number>>({});
  let trainers = $state<Entry[]>([]);
  let items = $state<Entry[]>([]);
  let allZones = $state<ZoneInfo[]>([]);
  let music = $state<SoundInfo[]>([]);
  let playing = $state<number | null>(null);
  // Object code -> its billboard texture member, for drawing people.
  let objTex = $state<(number | null)[]>([]);
  let spritePicker = $state(false);
  // What each event says and does, and text being edited (bank:id -> text).
  let scripts = $state<EventScript[]>([]);
  let lines = $state<Record<string, string>>({});
  let species = $state<Entry[]>([]);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let canvas = $state<HTMLCanvasElement | null>(null);
  let stage = $state<HTMLDivElement | null>(null);
  let space = $state(false);
  let audio: HTMLAudioElement | null = null;
  let clip: { w: number; h: number; cells: [number, number][] } | null = null;
  let eventClip: MapEvent | null = null;

  // What the pointer is doing.
  let drag = $state<
    | { t: "pan"; px: number; py: number; x: number; y: number }
    | { t: "event"; ev: MapEvent; x0: number; z0: number; ox: number; oz: number }
    | { t: "prop"; pr: Prop; before: Prop; ox: number; oz: number }
    | { t: "size"; ev: MapEvent }
    | { t: "marquee"; from: Pt }
    | { t: "tiles"; from: Pt; copy: boolean }
    | { t: "paint"; before: Map<number, [number, number]> }
    | { t: "box"; from: Pt }
    | null
  >(null);

  async function load(keepView = true) {
    error = null;
    try {
      const next = await invoke<ZoneMap>("zone_map", { zone: current });
      terrain = next.terrain;
      flags = next.flags;
      map = next;
      tick++;
      const keep = picked;
      picked = keep ? (map.events.find((e) => e.kind === keep.kind && e.index === keep.index) ?? null) : null;
      edits = {};
      if (!keepView) requestAnimationFrame(fit);
      loadScripts();
      loadProps();
    } catch (e) {
      error = String(e);
    }
  }

  async function loadProps() {
    try {
      placed = await invoke<Prop[]>("props", { zone: current });
      propModels = await invoke<number[]>("prop_models", { zone: current });
      if (newModel === null || !propModels.includes(newModel)) newModel = propModels[0] ?? null;
    } catch {
      placed = [];
    }
  }

  // Snaps to half tiles: models sit on tile centres or tile corners.
  const snap = (v: number) => Math.round(v * 2) / 2;

  async function saveProp(pr: Prop, x: number, z: number, y: number, rot: number, before?: Prop) {
    busy = true;
    try {
      const was = before ?? { ...pr };
      const [chunk, index] = await invoke<[number, number]>("set_prop", { zone: current, prop: was, x, z, y, rot });
      undo = [...undo.slice(-99), { t: "prop", chunk, index, x: was.x, z: was.z, y: was.y, rot: was.rot }];
      play("select");
      onSaved();
      renderStamp++;
      await loadProps();
      pickedProp = placed.find((q) => q.chunk === chunk && q.index === index) ?? null;
    } catch (err) {
      error = String(err);
      await loadProps();
    } finally {
      busy = false;
    }
  }

  async function addProp(p: Pt) {
    if (newModel === null) return;
    busy = true;
    try {
      const [chunk, index] = await invoke<[number, number]>("add_prop", {
        zone: current,
        model: newModel,
        x: p.x + 0.5,
        z: p.z + 0.5,
        y: 0,
        rot: 0,
      });
      play("decide");
      adding = null;
      onSaved();
      renderStamp++;
      await loadProps();
      pickedProp = placed.find((q) => q.chunk === chunk && q.index === index) ?? null;
      picked = null;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function copyProp(pr: Prop) {
    busy = true;
    try {
      const [chunk, index] = await invoke<[number, number]>("add_prop", {
        zone: current,
        model: pr.model,
        x: pr.x + 1,
        z: pr.z,
        y: pr.y,
        rot: pr.rot,
      });
      play("decide");
      onSaved();
      renderStamp++;
      await loadProps();
      pickedProp = placed.find((q) => q.chunk === chunk && q.index === index) ?? null;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function removeProp(pr: Prop) {
    busy = true;
    try {
      await invoke("remove_prop", { chunk: pr.chunk, index: pr.index });
      play("cancel");
      pickedProp = null;
      undo = undo.filter((s) => s.t !== "prop");
      onSaved();
      renderStamp++;
      await loadProps();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function onPropDown(pr: Prop, e: PointerEvent) {
    if (e.button !== 0 || space || adding || tool !== "select") return;
    e.stopPropagation();
    pickedProp = pr;
    picked = null;
    sel = null;
    play("select");
    if (readOnly) return;
    const r = stage!.getBoundingClientRect();
    const wx = (e.clientX - r.left - pan.x) / scale;
    const wz = (e.clientY - r.top - pan.y) / scale;
    drag = { t: "prop", pr, before: { ...pr }, ox: wx - pr.x, oz: wz - pr.z };
    stage!.setPointerCapture(e.pointerId);
  }

  async function loadScripts() {
    try {
      scripts = await invoke<EventScript[]>("event_scripts", { zone: current });
    } catch {
      scripts = [];
    }
  }

  // Text in the editor's form: real line breaks instead of "\n" escapes.
  function toEdit(text: string): string {
    return text.replace(/\\(.)/g, (m, c) => (c === "n" ? "\n" : m));
  }
  function fromEdit(text: string): string {
    return text.replace(/\r?\n/g, "\\n");
  }

  async function saveLine(m: MessageRef) {
    const key = `${m.bank}:${m.id}`;
    const text = lines[key];
    if (text === undefined) return;
    busy = true;
    try {
      const strings = await textBank(3, m.bank);
      strings[m.id] = fromEdit(text);
      await saveTextBank(3, m.bank, strings);
      delete lines[key];
      play("save");
      onSaved();
      await loadScripts();
    } catch (err) {
      error = String(err);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    load(false);
    tableEntries("trainers").then((t) => (trainers = t)).catch(() => {});
    tableEntries("items").then((t) => (items = t)).catch(() => {});
    tableEntries("species").then((t) => (species = t)).catch(() => {});
    invoke<ZoneInfo[]>("zones").then((z) => (allZones = z)).catch(() => {});
    once("objtex", () => invoke<(number | null)[]>("object_textures"))
      .then((t) => (objTex = t))
      .catch(() => {});
    sounds().then((s) => (music = s.filter((x) => x.group === "music"))).catch(() => {});
    return () => audio?.pause();
  });

  // The whole map in view, centred.
  function fit() {
    if (!stage || !map) return;
    const w = stage.clientWidth - 32;
    const h = stage.clientHeight - 32;
    scale = Math.max(minScale(), Math.min(32, Math.floor(Math.min(w / map.width, h / map.height))));
    setPan((stage.clientWidth - map.width * scale) / 2, (stage.clientHeight - map.height * scale) / 2);
  }

  // Zooming out stops when the map takes 80% of the view.
  function minScale(): number {
    if (!stage || !map) return 1;
    return Math.max(1, Math.min(stage.clientWidth / map.width, stage.clientHeight / map.height) * 0.8);
  }

  // Panning stops 20% of the view past each edge of the map.
  function setPan(x: number, y: number) {
    if (!stage || !map) return void (pan = { x, y });
    const clamp = (v: number, view: number, size: number) => {
      const margin = view * 0.2;
      let lo = view - margin - size;
      let hi = margin;
      if (lo > hi) {
        const mid = (view - size) / 2;
        [lo, hi] = [mid - margin, mid + margin];
      }
      return Math.max(lo, Math.min(hi, v));
    };
    pan = {
      x: clamp(x, stage.clientWidth, map.width * scale),
      y: clamp(y, stage.clientHeight, map.height * scale),
    };
  }

  function zoomAt(next: number, cx: number, cy: number) {
    next = Math.max(minScale(), Math.min(48, next));
    const wx = (cx - pan.x) / scale;
    const wy = (cy - pan.y) / scale;
    scale = next;
    setPan(cx - wx * next, cy - wy * next);
  }

  // Added by hand so it can stop the page scroll and zoom.
  $effect(() => {
    if (!stage) return;
    const el = stage;
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const r = stage!.getBoundingClientRect();
    if (e.ctrlKey || e.metaKey) {
      const step = Math.max(1, Math.round(scale * 0.15));
      zoomAt(scale + (e.deltaY < 0 ? step : -step), e.clientX - r.left, e.clientY - r.top);
    } else if (e.shiftKey) {
      setPan(pan.x - e.deltaY, pan.y);
    } else {
      setPan(pan.x - e.deltaX, pan.y - e.deltaY);
    }
  }

  // Tiles: one pixel each, scaled up by CSS.
  $effect(() => {
    void tick;
    if (!canvas || !map) return;
    const ctx = canvas.getContext("2d")!;
    const img = ctx.createImageData(map.width, map.height);
    const cache = new Map<number, [number, number, number]>();
    for (let i = 0; i < terrain.length; i++) {
      const t = terrain[i];
      if (t < 0) continue;
      const key = t * 0x10000 + flags[i];
      let c = cache.get(key);
      if (!c) cache.set(key, (c = tileRgb(t, flags[i])));
      img.data.set([c[0], c[1], c[2], 255], i * 4);
    }
    ctx.putImageData(img, 0, 0);
  });

  // The tile kinds this map uses, most common first.
  const palette = $derived.by(() => {
    void tick;
    const counts = new Map<number, number>();
    for (let i = 0; i < terrain.length; i++) {
      if (terrain[i] < 0) continue;
      const key = terrain[i] * 0x10000 + flags[i];
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return [...counts]
      .map(([key, count]) => ({ terrain: Math.floor(key / 0x10000), flags: key % 0x10000, count }))
      .sort((a, b) => b.count - a.count);
  });

  function at(e: PointerEvent | MouseEvent): Pt {
    const r = stage!.getBoundingClientRect();
    return { x: Math.floor((e.clientX - r.left - pan.x) / scale), z: Math.floor((e.clientY - r.top - pan.y) / scale) };
  }
  const inside = (p: Pt) => !!map && p.x >= 0 && p.z >= 0 && p.x < map.width && p.z < map.height;
  const inRect = (p: Pt, r: Rect | null) => !!r && p.x >= r.x && p.z >= r.z && p.x < r.x + r.w && p.z < r.z + r.h;
  const clampPt = (p: Pt): Pt => ({
    x: Math.max(0, Math.min(map!.width - 1, p.x)),
    z: Math.max(0, Math.min(map!.height - 1, p.z)),
  });
  const rectOf = (a: Pt, b: Pt): Rect => ({
    x: Math.min(a.x, b.x),
    z: Math.min(a.z, b.z),
    w: Math.abs(a.x - b.x) + 1,
    h: Math.abs(a.z - b.z) + 1,
  });

  // ---- pointer on the map ----

  function onDown(e: PointerEvent) {
    if (!map) return;
    const target = e.currentTarget as HTMLElement;
    if (e.button === 1 || (e.button === 0 && space)) {
      e.preventDefault();
      drag = { t: "pan", px: e.clientX, py: e.clientY, x: pan.x, y: pan.y };
      target.setPointerCapture(e.pointerId);
      return;
    }
    const p = at(e);
    if (e.button === 2) {
      if (inside(p) && terrain[p.z * map.width + p.x] >= 0) {
        const i = p.z * map.width + p.x;
        brush = { terrain: terrain[i], flags: flags[i] };
        if (tool === "select") tool = "paint";
        play("select");
      }
      return;
    }
    if (e.button !== 0 || !inside(p)) return;
    if (adding) {
      if (!readOnly) {
        if (adding === "prop") addProp(p);
        else addEvent(adding, p);
      }
      return;
    }
    const i = p.z * map.width + p.x;
    switch (tool) {
      case "select":
        picked = null;
        pickedProp = null;
        if (inRect(p, sel) && !readOnly) {
          drag = { t: "tiles", from: p, copy: e.altKey };
          moving = { x: 0, z: 0 };
        } else {
          sel = { x: p.x, z: p.z, w: 1, h: 1 };
          drag = { t: "marquee", from: p };
        }
        break;
      case "pick":
        if (terrain[i] >= 0) brush = { terrain: terrain[i], flags: flags[i] };
        tool = "paint";
        play("select");
        return;
      case "fill":
        if (readOnly) return;
        drag = { t: "paint", before: new Map() };
        flood(p);
        finishPaint();
        return;
      case "box":
        if (readOnly) return;
        drag = { t: "box", from: p };
        boxing = rectOf(p, p);
        break;
      case "paint":
        if (readOnly) return;
        drag = { t: "paint", before: new Map() };
        touch(i);
        break;
    }
    target.setPointerCapture(e.pointerId);
  }

  function onMove(e: PointerEvent) {
    const r = stage!.getBoundingClientRect();
    mouse = { x: e.clientX - r.left, y: e.clientY - r.top };
    const p = at(e);
    hover = inside(p) ? p : null;
    if (!drag || !map) return;
    switch (drag.t) {
      case "pan":
        setPan(drag.x + e.clientX - drag.px, drag.y + e.clientY - drag.py);
        break;
      case "event": {
        const ev = drag.ev;
        ev.x = Math.max(0, Math.min(map.width - ev.w, p.x - drag.ox));
        ev.z = Math.max(0, Math.min(map.height - ev.h, p.z - drag.oz));
        break;
      }
      case "prop": {
        const r = stage!.getBoundingClientRect();
        drag.pr.x = snap((e.clientX - r.left - pan.x) / scale - drag.ox);
        drag.pr.z = snap((e.clientY - r.top - pan.y) / scale - drag.oz);
        break;
      }
      case "size":
        drag.ev.w = Math.max(1, p.x - drag.ev.x + 1);
        drag.ev.h = Math.max(1, p.z - drag.ev.z + 1);
        break;
      case "marquee":
        sel = rectOf(drag.from, clampPt(p));
        break;
      case "tiles":
        moving = { x: p.x - drag.from.x, z: p.z - drag.from.z };
        break;
      case "paint":
        if (inside(p)) touch(p.z * map.width + p.x);
        break;
      case "box":
        boxing = rectOf(drag.from, clampPt(p));
        break;
    }
  }

  async function onUp() {
    const d = drag;
    if (!d || !map) return;
    if (d.t === "paint") return finishPaint();
    drag = null;
    switch (d.t) {
      case "event":
        if (d.ev.x !== d.x0 || d.ev.z !== d.z0) await moveEvent(d.ev, d.x0, d.z0);
        break;
      case "prop":
        if (d.pr.x !== d.before.x || d.pr.z !== d.before.z) await saveProp(d.pr, d.pr.x, d.pr.z, d.pr.y, d.pr.rot, d.before);
        break;
      case "size":
        await save(d.ev, { sizeX: d.ev.w, sizeZ: d.ev.h }, false);
        break;
      case "marquee":
        if (sel && sel.w * sel.h > 1) play("select");
        break;
      case "tiles": {
        const m = moving;
        moving = null;
        if (m && (m.x || m.z)) await moveTiles(m, d.copy);
        break;
      }
      case "box":
        if (boxing) {
          const b = boxing;
          boxing = null;
          drag = { t: "paint", before: new Map() };
          for (let z = b.z; z < b.z + b.h; z++) for (let x = b.x; x < b.x + b.w; x++) touch(z * map.width + x);
          finishPaint();
        }
        break;
    }
  }

  // ---- tiles ----

  function touch(i: number) {
    if (drag?.t !== "paint" || terrain[i] < 0) return;
    if (terrain[i] === brush.terrain && flags[i] === brush.flags) return;
    if (!drag.before.has(i)) drag.before.set(i, [terrain[i], flags[i]]);
    terrain[i] = brush.terrain;
    flags[i] = brush.flags;
    tick++;
  }

  function flood(p: Pt) {
    const w = map!.width;
    const start = p.z * w + p.x;
    const from = [terrain[start], flags[start]];
    if (from[0] === brush.terrain && from[1] === brush.flags) return;
    const seen = new Set([start]);
    const todo = [start];
    while (todo.length) {
      const i = todo.pop()!;
      touch(i);
      const x = i % w;
      for (const j of [i - w, i + w, x > 0 ? i - 1 : -1, x < w - 1 ? i + 1 : -1]) {
        if (j < 0 || j >= terrain.length || seen.has(j)) continue;
        if (terrain[j] !== from[0] || flags[j] !== from[1]) continue;
        seen.add(j);
        todo.push(j);
      }
    }
  }

  function finishPaint() {
    const d = drag;
    drag = null;
    if (d?.t !== "paint" || !d.before.size || !map) return;
    const w = map.width;
    const before: TileWrite[] = [...d.before].map(([i, [t, f]]) => [i % w, Math.floor(i / w), t, f]);
    const after: TileWrite[] = [...d.before.keys()].map((i) => [i % w, Math.floor(i / w), terrain[i], flags[i]]);
    writeTiles(after, before);
  }

  // Writes tiles already changed on screen; `before` makes the undo step.
  async function writeTiles(after: TileWrite[], before: TileWrite[] | null) {
    if (before) undo = [...undo.slice(-99), { t: "tiles", before }];
    try {
      await invoke("write_tiles", { zone: current, tiles: after });
      play("select");
      onSaved();
    } catch (err) {
      error = String(err);
      await load();
    }
  }

  function setLocal(writes: TileWrite[]): TileWrite[] {
    const w = map!.width;
    const before: TileWrite[] = [];
    for (const [x, z, t, f] of writes) {
      const i = z * w + x;
      if (terrain[i] < 0) continue;
      before.push([x, z, terrain[i], flags[i]]);
      terrain[i] = t;
      flags[i] = f;
    }
    tick++;
    return before;
  }

  // Moves (or copies) the selected tiles and the things on them; the
  // ground left behind takes the paint tile.
  async function moveTiles(d: Pt, copy: boolean) {
    if (!sel || !map) return;
    const w = map.width;
    const src = sel;
    const dst = { ...src, x: src.x + d.x, z: src.z + d.z };
    const writes = new Map<number, TileWrite>();
    if (!copy)
      for (let z = src.z; z < src.z + src.h; z++)
        for (let x = src.x; x < src.x + src.w; x++) writes.set(z * w + x, [x, z, brush.terrain, brush.flags]);
    for (let z = 0; z < src.h; z++)
      for (let x = 0; x < src.w; x++) {
        const [sx, sz, tx, tz] = [src.x + x, src.z + z, dst.x + x, dst.z + z];
        if (!inside({ x: tx, z: tz })) continue;
        const i = sz * w + sx;
        if (terrain[i] < 0) continue;
        writes.set(tz * w + tx, [tx, tz, terrain[i], flags[i]]);
      }
    const list = [...writes.values()];
    const before = setLocal(list);
    sel = dst;
    await writeTiles(list, before);
    if (!copy)
      for (const ev of map.events.filter((e) => inRect(e, src))) {
        const [x0, z0] = [ev.x, ev.z];
        ev.x = Math.max(0, Math.min(map.width - 1, ev.x + d.x));
        ev.z = Math.max(0, Math.min(map.height - 1, ev.z + d.z));
        await moveEvent(ev, x0, z0);
      }
  }

  function selTiles(f: (i: number, x: number, z: number) => TileWrite | null): TileWrite[] {
    const list: TileWrite[] = [];
    if (!sel || !map) return list;
    for (let z = sel.z; z < sel.z + sel.h; z++)
      for (let x = sel.x; x < sel.x + sel.w; x++) {
        const i = z * map.width + x;
        if (terrain[i] < 0) continue;
        const w = f(i, x, z);
        if (w) list.push(w);
      }
    return list;
  }

  function applySelection(f: (i: number, x: number, z: number) => TileWrite | null) {
    if (readOnly) return;
    const list = selTiles(f);
    if (!list.length) return;
    const before = setLocal(list);
    writeTiles(list, before);
  }

  // The kinds in the selection, for its summary.
  const selKinds = $derived.by(() => {
    void tick;
    if (!sel || !map) return [] as { terrain: number; count: number }[];
    const counts = new Map<number, number>();
    for (let z = sel.z; z < sel.z + sel.h; z++)
      for (let x = sel.x; x < sel.x + sel.w; x++) {
        const t = terrain[z * map.width + x];
        if (t >= 0) counts.set(t, (counts.get(t) ?? 0) + 1);
      }
    return [...counts].map(([terrain, count]) => ({ terrain, count })).sort((a, b) => b.count - a.count);
  });

  // ---- events ----

  function onEventDown(ev: MapEvent, e: PointerEvent) {
    if (e.button !== 0 || space || adding || tool !== "select") return;
    e.stopPropagation();
    picked = ev;
    sel = null;
    edits = {};
    play("select");
    if (readOnly) return;
    const p = at(e);
    drag = { t: "event", ev, x0: ev.x, z0: ev.z, ox: p.x - ev.x, oz: p.z - ev.z };
    stage!.setPointerCapture(e.pointerId);
  }

  function onSizeDown(ev: MapEvent, e: PointerEvent) {
    if (e.button !== 0 || readOnly) return;
    e.stopPropagation();
    drag = { t: "size", ev };
    stage!.setPointerCapture(e.pointerId);
  }

  async function moveEvent(ev: MapEvent, x0: number, z0: number) {
    try {
      await invoke("move_event", { zone: current, kind: ev.kind, index: ev.index, x: ev.x, z: ev.z });
      undo = [...undo.slice(-99), { t: "move", kind: ev.kind, index: ev.index, x: x0, z: z0 }];
      play("select");
      onSaved();
      refreshValues();
    } catch (err) {
      error = String(err);
      await load();
    }
  }

  // Re-reads the event values after a change without touching the view.
  async function refreshValues() {
    try {
      const next = await invoke<ZoneMap>("zone_map", { zone: current });
      if (!map) return;
      for (const ev of map.events) {
        const n = next.events.find((e) => e.kind === ev.kind && e.index === ev.index);
        if (n) {
          ev.values = n.values;
          ev.trainer = n.trainer;
          ev.toZone = n.toZone;
        }
      }
    } catch {
      // The next full load picks it up.
    }
  }

  async function addEvent(what: Add, p: Pt) {
    busy = true;
    const kind: Kind = what === "item" ? "bg" : (what as Kind);
    try {
      const index = await invoke<number>("add_event", { zone: current, kind, x: p.x, z: p.z });
      if (what === "item") await invoke("save_event", { zone: current, kind, index, values: { type: 2, scriptId: 0 } });
      if (what === "bg") await invoke("save_event", { zone: current, kind, index, values: { type: 0 } });
      undo = [...undo.slice(-99), { t: "add", kind, index }];
      play("decide");
      adding = null;
      await load();
      picked = map!.events.find((ev) => ev.kind === kind && ev.index === index) ?? null;
      onSaved();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function duplicate(ev: MapEvent, p?: Pt) {
    if (readOnly || !map) return;
    busy = true;
    try {
      const to = p ?? { x: Math.min(map.width - 1, ev.x + 1), z: ev.z };
      const index = await invoke<number>("copy_event", { zone: current, kind: ev.kind, index: ev.index, x: to.x, z: to.z });
      undo = [...undo.slice(-99), { t: "add", kind: ev.kind, index }];
      play("decide");
      await load();
      picked = map!.events.find((e) => e.kind === ev.kind && e.index === index) ?? null;
      onSaved();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function save(ev: MapEvent, values: Record<string, number>, reload = true) {
    busy = true;
    try {
      await invoke("save_event", { zone: current, kind: ev.kind, index: ev.index, values });
      play("save");
      onSaved();
      if (reload) await load();
      else await refreshValues();
    } catch (err) {
      error = String(err);
      play("cancel");
      await load();
    } finally {
      busy = false;
    }
  }

  async function setTrainer(ev: MapEvent, trainer: number | null) {
    busy = true;
    try {
      await invoke("set_trainer", { zone: current, index: ev.index, trainer });
      play("save");
      onSaved();
      await load();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function remove(ev: MapEvent) {
    busy = true;
    try {
      await invoke("remove_event", { zone: current, kind: ev.kind, index: ev.index });
      play("cancel");
      picked = null;
      // Indices after it shift down: older event undo steps no longer apply.
      undo = undo.filter((s) => s.t === "tiles");
      await load();
      onSaved();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function undoStep() {
    const step = undo.at(-1);
    if (!step || readOnly || !map) return;
    undo = undo.slice(0, -1);
    try {
      if (step.t === "tiles") {
        setLocal(step.before);
        await invoke("write_tiles", { zone: current, tiles: step.before });
      } else if (step.t === "prop") {
        const pr = placed.find((q) => q.chunk === step.chunk && q.index === step.index);
        if (pr) {
          const [chunk, index] = await invoke<[number, number]>("set_prop", {
            zone: current,
            prop: pr,
            x: step.x,
            z: step.z,
            y: step.y,
            rot: step.rot,
          });
          renderStamp++;
          await loadProps();
          if (pickedProp) pickedProp = placed.find((q) => q.chunk === chunk && q.index === index) ?? null;
        }
      } else if (step.t === "move") {
        const ev = map.events.find((e) => e.kind === step.kind && e.index === step.index);
        if (ev) {
          ev.x = step.x;
          ev.z = step.z;
        }
        await invoke("move_event", { zone: current, kind: step.kind, index: step.index, x: step.x, z: step.z });
      } else {
        await invoke("remove_event", { zone: current, kind: step.kind, index: step.index });
        if (picked?.kind === step.kind && picked.index === step.index) picked = null;
        await load();
      }
      play("cancel");
      onSaved();
    } catch (err) {
      error = String(err);
      await load();
    }
  }

  // ---- zone ----

  async function saveMusic(season: number, seq: number) {
    if (!map) return;
    const next = [...map.zone.music];
    next[season] = seq;
    try {
      await invoke("set_music", { zone: current, music: next });
      map.zone.music = next;
      play("save");
      onSaved();
    } catch (err) {
      error = String(err);
    }
  }

  function toggleMusic(season: number) {
    audio?.pause();
    audio = null;
    if (playing === season || !map) {
      playing = null;
      return;
    }
    playing = season;
    const a = new Audio(asset(`sound/${map.zone.music[season]}?seconds=60`));
    a.onended = () => {
      if (audio === a) playing = null;
    };
    audio = a;
    a.play().catch(() => (playing = null));
  }

  function switchZone(id: number) {
    play("select");
    current = id;
    picked = null;
    sel = null;
    undo = [];
    renderOk = true;
    audio?.pause();
    playing = null;
    load(false);
  }

  // ---- keys ----

  function onKey(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input, select, textarea, .monaco-editor")) return;
    if (e.key === " ") {
      space = e.type === "keydown";
      e.preventDefault();
      return;
    }
    if (e.type !== "keydown" || !map) return;
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if (mod && k === "z") {
      e.preventDefault();
      undoStep();
      return;
    }
    if (mod && k === "a") {
      e.preventDefault();
      tool = "select";
      picked = null;
      sel = { x: 0, z: 0, w: map.width, h: map.height };
      return;
    }
    if (mod && k === "c") {
      if (picked) eventClip = picked;
      else if (sel) {
        const cells: [number, number][] = [];
        for (let z = sel.z; z < sel.z + sel.h; z++)
          for (let x = sel.x; x < sel.x + sel.w; x++) cells.push([terrain[z * map.width + x], flags[z * map.width + x]]);
        clip = { w: sel.w, h: sel.h, cells };
        eventClip = null;
      }
      play("select");
      return;
    }
    if (mod && k === "v" && hover && !readOnly) {
      const h = hover;
      if (eventClip) duplicate(eventClip, h);
      else if (clip) {
        const c = clip;
        const list: TileWrite[] = [];
        c.cells.forEach(([t, f], i) => {
          const p = { x: h.x + (i % c.w), z: h.z + Math.floor(i / c.w) };
          if (t >= 0 && inside(p)) list.push([p.x, p.z, t, f]);
        });
        const before = setLocal(list);
        sel = { x: h.x, z: h.z, w: c.w, h: c.h };
        writeTiles(list, before);
      }
      return;
    }
    if (mod && k === "d" && picked) {
      e.preventDefault();
      duplicate(picked);
      return;
    }
    if (mod) return;
    if (e.key === "Escape") {
      pickedProp = null;
      picked = null;
      sel = null;
      adding = null;
      return;
    }
    if ((e.key === "Delete" || e.key === "Backspace") && !readOnly) {
      if (pickedProp) removeProp(pickedProp);
      else if (picked) remove(picked);
      else if (sel) applySelection((_, x, z) => [x, z, brush.terrain, brush.flags]);
      return;
    }
    const t = TOOLS.find((x) => x[2].toLowerCase() === k);
    if (t) {
      tool = t[0];
      adding = null;
    } else if (k === "h") grid = !grid;
    else if (k === "f") fit();
  }

  // ---- labels ----

  const trainerName = (id: number) => trainers[id]?.name || `Trainer ${id}`;
  const itemName = (id: number) => items[id]?.name || `Item ${id}`;
  const zoneName = (id: number) => {
    const z = allZones[id];
    return z ? `${z.place} (${id})` : `Map ${id}`;
  };
  const musicName = (seq: number) => music.find((m) => m.index === seq)?.name ?? `#${seq}`;
  const value = (ev: MapEvent, key: string) => ev.values.find((v) => v.key === key)?.value ?? 0;
  const isItem = (ev: MapEvent) => ev.kind === "bg" && value(ev, "type") === 2;
  function eventLabel(ev: MapEvent): string {
    if (ev.trainer !== null) return trainerName(ev.trainer);
    if (isItem(ev)) return `Hidden ${itemName(value(ev, "scriptId"))}`;
    if (ev.kind === "warp" && ev.toZone !== null) return `Warp to ${zoneName(ev.toZone)}`;
    return `${KIND_LABEL[ev.kind]} ${ev.index + 1}`;
  }
  // A person's picture: the frame facing the way they look.
  function sprite(ev: MapEvent): string | null {
    if (ev.kind !== "npc") return null;
    return spriteOf(value(ev, "objCode"), value(ev, "dirHeader"));
  }
  function spriteOf(code: number, dir = 1): string | null {
    const tex = objTex[code];
    if (tex === null || tex === undefined) return null;
    return asset(`texture/49:${tex}/0/${[0, 3, 6, 9][dir] ?? 3}`);
  }
  const scriptOf = (ev: MapEvent) =>
    ev.trainer !== null ? null : (scripts.find((s) => s.kind === ev.kind && s.index === ev.index)?.script ?? null);
  // A message once, even when several paths of a script show it.
  const uniqueMessages = (sc: ScriptInfo) => sc.messages.filter((m, i, all) => all.findIndex((n) => n.bank === m.bank && n.id === m.id) === i);
  const eventIcon = (ev: MapEvent) =>
    ev.trainer !== null ? "T" : ev.kind === "npc" ? "P" : ev.kind === "warp" ? "W" : ev.kind === "pos" ? "" : isItem(ev) ? "I" : "S";

  // The spot the preview looks at: the selected thing, the selection, else
  // the middle of the view.
  const focus = $derived.by(() => {
    if (picked) return { x: picked.x, z: picked.z };
    if (sel) return { x: sel.x + Math.floor(sel.w / 2), z: sel.z + Math.floor(sel.h / 2) };
    if (!map || !stage) return { x: 0, z: 0 };
    return {
      x: Math.max(0, Math.min(map.width - 1, Math.floor((stage.clientWidth / 2 - pan.x) / scale))),
      z: Math.max(0, Math.min(map.height - 1, Math.floor((stage.clientHeight / 2 - pan.y) / scale))),
    };
  });
  // The preview follows the focus once it settles.
  let previewAt = $state({ x: 0, z: 0 });
  $effect(() => {
    const f = focus;
    if (!preview) return;
    const t = setTimeout(() => (previewAt = f), 250);
    return () => clearTimeout(t);
  });
  const hoverIndex = $derived(hover && map ? hover.z * map.width + hover.x : -1);
  const counts = $derived.by(() => {
    const ev = map?.events ?? [];
    return [
      ["People", ev.filter((e) => e.kind === "npc" && e.trainer === null)],
      ["Trainers", ev.filter((e) => e.trainer !== null)],
      ["Signs", ev.filter((e) => e.kind === "bg" && !isItem(e))],
      ["Hidden items", ev.filter(isItem)],
      ["Warps", ev.filter((e) => e.kind === "warp")],
      ["Triggers", ev.filter((e) => e.kind === "pos")],
    ] as [string, MapEvent[]][];
  });
  const painting = $derived(tool !== "select");
  const showTiles = $derived(look === "tiles" || !renderOk || painting || sel !== null);
  const commonKinds = $derived([...new Set([...palette.map((p) => p.terrain), 0, 4, 6, 8, 0x0c, 0x3d, 0x3f, 0x72, 0x73, 0x74, 0x75])]);
  const hint = $derived(
    adding ? `Click on the map to place a ${ADDS.find((a) => a[0] === adding)![1].toLowerCase()}. Esc cancels.` : HINT[tool],
  );
</script>

<svelte:window onkeydown={onKey} onkeyup={onKey} />

<div class="zone-editor">
  <header class="bar">
    <button onclick={() => (play("cancel"), onBack())}>World map</button>
    <h2>{map?.zone.place ?? ""}</h2>
    {#if zones.length > 1}
      <select value={current} onchange={(e) => switchZone(Number((e.currentTarget as HTMLSelectElement).value))} aria-label="Map">
        {#each zones as z}<option value={z.id}>{z.label}</option>{/each}
      </select>
    {/if}
    <div class="spacer"></div>
    {#if renderOk}
      <div class="seg" role="radiogroup" aria-label="Look">
        <button class:primary={look === "game"} onclick={() => (look = "game")}>Game</button>
        <button class:primary={look === "tiles"} onclick={() => (look = "tiles")}>Tile kinds</button>
      </div>
    {/if}
    <div class="zoomer">
      <button class="quiet" onclick={() => stage && zoomAt(scale - Math.max(1, Math.round(scale * 0.2)), stage.clientWidth / 2, stage.clientHeight / 2)} aria-label="Zoom out">-</button>
      <button class="quiet zoom" onclick={fit} title="Fit (F)">{Math.round((scale / 16) * 100)}%</button>
      <button class="quiet" onclick={() => stage && zoomAt(scale + Math.max(1, Math.round(scale * 0.2)), stage.clientWidth / 2, stage.clientHeight / 2)} aria-label="Zoom in">+</button>
    </div>
    <button class:primary={grid} onclick={() => (grid = !grid)} title="Grid (H)">Grid</button>
    <button class:primary={showProps} onclick={() => (showProps = !showProps)} title="Show buildings, trees and other objects">Objects</button>
    <button class:primary={preview} onclick={() => (preview = !preview)} title="See the spot as the game shows it">Preview</button>
    <button disabled={!undo.length || readOnly} onclick={undoStep} title="Undo (Ctrl+Z)">Undo</button>
  </header>

  {#if error}<p class="bad pad" role="alert">{error}</p>{/if}

  <div class="body">
    <aside class="tools">
      {#each TOOLS as [t, label, key]}
        <button
          class="tool"
          class:primary={tool === t && !adding}
          disabled={readOnly && t !== "select"}
          title="{label} ({key})"
          onclick={() => ((tool = t), (adding = null), play("select"))}
        >{label}<kbd>{key}</kbd></button>
      {/each}
      {#if !readOnly}
        <h3>Add</h3>
        {#each ADDS as [a, label]}
          <button class="tool" class:primary={adding === a} onclick={() => ((adding = adding === a ? null : a), play("select"))}>
            <i class="dot {a}"></i>{label}
          </button>
        {/each}
      {/if}
      {#if adding === "prop" && propModels.length}
        <h3>Object to place</h3>
        <select bind:value={newModel} aria-label="Object to place">
          {#each propModels as m}<option value={m}>Object {m}</option>{/each}
        </select>
      {/if}
      {#if painting}
        <h3>Paint with</h3>
        <div class="brush">
          <span class="chip big" style="background: {tileCss(brush.terrain, brush.flags)}"></span>
          <span class="brush-name">{tileLabel(brush.terrain, brush.flags)}</span>
        </div>
        <label class="check"><input type="checkbox" checked={(brush.flags & FLAG_BLOCKED) !== 0} onchange={() => (brush.flags ^= FLAG_BLOCKED)} />Blocked</label>
        <div class="kinds">
          {#each palette as p (p.terrain * 0x10000 + p.flags)}
            <button
              class="kind"
              class:on={p.terrain === brush.terrain && p.flags === brush.flags}
              title="{p.count} tiles"
              onclick={() => ((brush = { terrain: p.terrain, flags: p.flags }), play("select"))}
            >
              <span class="chip" style="background: {tileCss(p.terrain, p.flags)}"></span>
              <span class="kind-name">{tileLabel(p.terrain, p.flags)}</span>
            </button>
          {/each}
        </div>
      {/if}
    </aside>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="stage"
      class:panning={space || drag?.t === "pan"}
      bind:this={stage}
      onpointerdown={onDown}
      onpointermove={onMove}
      onpointerup={onUp}
      onpointerleave={() => ((hover = null), (hoverEvent = null))}
      oncontextmenu={(e) => e.preventDefault()}
      onauxclick={(e) => e.preventDefault()}
    >
      {#if map}
        <div class="world" style="width: {map.width * scale}px; height: {map.height * scale}px; transform: translate({pan.x}px, {pan.y}px)">
          {#if renderOk}
            <!-- A small picture first, the sharp one over it once drawn. -->
            {#key `${current}/${renderStamp}`}
              <img
                class="render"
                class:hidden={look === "tiles"}
                src={asset(`maprender/${current}?px=4&ss=1${renderStamp ? `&v=${renderStamp}` : ""}`)}
                alt=""
                draggable="false"
                onerror={() => (renderOk = false)}
              />
              <img
                class="render sharp"
                class:hidden={look === "tiles"}
                src={asset(`maprender/${current}?px=16${renderStamp ? `&v=${renderStamp}` : ""}`)}
                alt=""
                draggable="false"
                onload={(e) => (e.currentTarget as HTMLImageElement).classList.add("ready")}
              />
            {/key}
          {/if}
          <canvas
            bind:this={canvas}
            width={map.width}
            height={map.height}
            class="tiles"
            style="opacity: {showTiles ? (renderOk && look === 'game' ? 0.55 : 1) : 0}"
          ></canvas>
          {#if grid && scale >= 6}
            <div class="grid" style="--s: {scale}px; --c: {scale * 32}px"></div>
          {/if}
          {#if showProps || adding === "prop" || pickedProp}
            {#each placed as pr (`${pr.chunk}:${pr.index}`)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="prop"
                class:on={pickedProp === pr}
                class:passive={tool !== "select" || adding !== null}
                style="left: {(pr.x - 0.5) * scale}px; top: {(pr.z - 0.5) * scale}px; width: {scale}px; height: {scale}px"
                title="Object {pr.model}"
                onpointerdown={(e) => onPropDown(pr, e)}
              ></div>
            {/each}
          {/if}
          {#each map.events as ev (ev.kind + ev.index)}
            {#if ev.x >= 0 && ev.z >= 0}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              {@const pic = sprite(ev)}
              <div
                class="ev {ev.kind}"
                class:sprite={pic !== null}
                class:trainer={ev.trainer !== null}
                class:item={isItem(ev)}
                class:on={picked === ev}
                class:passive={tool !== "select" || adding !== null}
                style="left: {ev.x * scale}px; top: {ev.z * scale}px; width: {ev.w * scale}px; height: {ev.h * scale}px; font-size: {Math.max(8, scale * 0.6)}px"
                onpointerdown={(e) => onEventDown(ev, e)}
                onpointerenter={() => (hoverEvent = ev)}
                onpointerleave={() => (hoverEvent = null)}
              >
                {#if pic}
                  <img class="person" src={pic} alt="" draggable="false" style="width: {scale * 2}px; height: {scale * 2}px; left: {-scale / 2}px" />
                  {#if ev.trainer !== null}<span class="flag">!</span>{/if}
                {:else}
                  {eventIcon(ev)}
                {/if}
                {#if picked === ev && ev.kind === "pos" && !readOnly}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <span class="handle" onpointerdown={(e) => onSizeDown(ev, e)}></span>
                {/if}
              </div>
            {/if}
          {/each}
          {#if sel}
            <div
              class="sel"
              class:moving={moving !== null}
              style="left: {(sel.x + (moving?.x ?? 0)) * scale}px; top: {(sel.z + (moving?.z ?? 0)) * scale}px; width: {sel.w * scale}px; height: {sel.h * scale}px"
            ></div>
          {/if}
          {#if boxing}
            <div class="sel box" style="left: {boxing.x * scale}px; top: {boxing.z * scale}px; width: {boxing.w * scale}px; height: {boxing.h * scale}px"></div>
          {/if}
          {#if hover && (painting || adding)}
            <div class="cursor" style="left: {hover.x * scale}px; top: {hover.z * scale}px; width: {scale}px; height: {scale}px"></div>
          {/if}
        </div>
        {#if (hoverEvent || hoverIndex >= 0) && !drag}
          <div class="tip window" style="left: {mouse.x + 14}px; top: {mouse.y + 14}px">
            {#if hoverEvent}
              <strong>{eventLabel(hoverEvent)}</strong>
            {:else}
              {tileLabel(terrain[hoverIndex], flags[hoverIndex])} <span class="muted">{hover?.x}, {hover?.z}</span>
            {/if}
          </div>
        {/if}
        {#if adding}
          <div class="banner window">{hint}</div>
        {/if}
        {#if preview}
          <figure class="preview window">
            <img src={asset(`maprender/${current}?view=camera&x=${previewAt.x}&z=${previewAt.z}&w=384&h=288`)} alt="In game" draggable="false" />
            <figcaption class="muted">{previewAt.x}, {previewAt.z}</figcaption>
          </figure>
        {/if}
      {:else}
        <p class="muted pad">Loading...</p>
      {/if}
    </div>

    <aside class="panel">
      {#if pickedProp && map}
        {@const pr = pickedProp}
        <div class="head">
          <span class="badge prop-badge">O</span>
          <div class="title">
            <strong>Object {pr.model}</strong>
            <span class="muted">at {pr.x}, {pr.z}</span>
          </div>
        </div>
        {#if !readOnly}
          <div class="row">
            <button onclick={() => copyProp(pr)} disabled={busy}>Duplicate</button>
            <button onclick={() => removeProp(pr)} disabled={busy}>Delete</button>
          </div>
          <label class="field">
            <span>Model</span>
            <select
              value={pr.model}
              disabled={busy}
              onchange={async (e) => {
                const model = Number((e.currentTarget as HTMLSelectElement).value);
                busy = true;
                try {
                  await invoke("remove_prop", { chunk: pr.chunk, index: pr.index });
                  const [chunk, index] = await invoke<[number, number]>("add_prop", { zone: current, model, x: pr.x, z: pr.z, y: pr.y, rot: pr.rot });
                  play("save");
                  onSaved();
                  renderStamp++;
                  await loadProps();
                  pickedProp = placed.find((q) => q.chunk === chunk && q.index === index) ?? null;
                } catch (err) {
                  error = String(err);
                } finally {
                  busy = false;
                }
              }}
            >
              {#each propModels as m}<option value={m}>Object {m}</option>{/each}
            </select>
          </label>
          <div class="field">
            <span>Turn</span>
            <div class="row">
              {#each [0, 90, 180, 270] as deg}
                <button class:primary={Math.round(pr.rot) % 360 === deg} disabled={busy} onclick={() => saveProp(pr, pr.x, pr.z, pr.y, deg)}>{deg}°</button>
              {/each}
            </div>
          </div>
          <div class="row">
            <label class="field">
              <span>Across</span>
              <input type="number" step="0.5" value={pr.x} onchange={(e) => saveProp(pr, Number((e.currentTarget as HTMLInputElement).value), pr.z, pr.y, pr.rot)} />
            </label>
            <label class="field">
              <span>Down</span>
              <input type="number" step="0.5" value={pr.z} onchange={(e) => saveProp(pr, pr.x, Number((e.currentTarget as HTMLInputElement).value), pr.y, pr.rot)} />
            </label>
            <label class="field">
              <span>Height</span>
              <input type="number" step="0.5" value={pr.y} onchange={(e) => saveProp(pr, pr.x, pr.z, Number((e.currentTarget as HTMLInputElement).value), pr.rot)} />
            </label>
          </div>
          <p class="muted small">Drag it on the map to move it.</p>
        {/if}
      {:else if picked && map}
        {@const ev = picked}
        <div class="head">
          {#if sprite(ev)}
            <img class="badge pic" src={sprite(ev)} alt="" />
          {:else}
            <span class="badge {ev.kind}" class:trainer={ev.trainer !== null} class:item={isItem(ev)}>{eventIcon(ev) || "+"}</span>
          {/if}
          <div class="title">
            <strong>{eventLabel(ev)}</strong>
            <span class="muted">{ev.trainer !== null ? "Trainer" : isItem(ev) ? "Hidden item" : KIND_LABEL[ev.kind]} at {ev.x}, {ev.z}</span>
          </div>
        </div>
        {#if !readOnly}
          <div class="row">
            <button onclick={() => duplicate(ev)} disabled={busy} title="Ctrl+D">Duplicate</button>
            <button onclick={() => remove(ev)} disabled={busy} title="Delete">Delete</button>
          </div>
        {/if}

        {@const tabs = [
          ["main", ev.trainer !== null ? "Person" : KIND_LABEL[ev.kind]],
          ...(scriptOf(ev) ? [["says", "Says"]] : []),
          ...(ev.trainer !== null ? [["trainer", "Trainer"]] : []),
          ["values", "Values"],
        ]}
        <div class="ptabs" role="tablist">
          {#each tabs as [id, label]}
            <button role="tab" aria-selected={ptab === id} class:primary={ptab === id} onclick={() => (ptab = id)}>{label}</button>
          {/each}
        </div>
        {#if ptab === "main"}
        {#if ev.kind === "npc"}
          <label class="field">
            <span>Battles</span>
            <select
              value={ev.trainer ?? -1}
              disabled={readOnly || busy}
              onchange={(e) => {
                const v = Number((e.currentTarget as HTMLSelectElement).value);
                setTrainer(ev, v < 0 ? null : v);
              }}
            >
              <option value={-1}>Nobody</option>
              {#each trainers as t}{#if t.index > 0}<option value={t.index}>{t.name || `Trainer ${t.index}`} ({t.index})</option>{/if}{/each}
            </select>
          </label>
          <div class="field">
            <span>Looks like</span>
            <button class="look" disabled={readOnly} onclick={() => (spritePicker = !spritePicker)}>
              {#if sprite(ev)}<img src={sprite(ev)} alt="" />{/if}
              <span>Character {value(ev, "objCode")}</span>
            </button>
          </div>
          {#if spritePicker && !readOnly}
            <div class="sprites">
              {#each objTex as tex, code}
                {#if tex !== null}
                  <button
                    class="sprite-pick"
                    class:on={code === value(ev, "objCode")}
                    title="Character {code}"
                    onclick={() => {
                      spritePicker = false;
                      save(ev, { objCode: code }, false);
                    }}
                  ><img src={spriteOf(code)} alt="Character {code}" loading="lazy" /></button>
                {/if}
              {/each}
            </div>
          {/if}
          <label class="field">
            <span>Facing</span>
            <select value={value(ev, "dirHeader")} disabled={readOnly || busy} onchange={(e) => save(ev, { dirHeader: Number((e.currentTarget as HTMLSelectElement).value) }, false)}>
              {#each ["Up", "Down", "Left", "Right"] as d, i}<option value={i}>{d}</option>{/each}
            </select>
          </label>
        {:else if ev.kind === "warp"}
          <label class="field">
            <span>Leads to</span>
            <select value={ev.toZone ?? 0} disabled={readOnly || busy} onchange={(e) => save(ev, { zoneId: Number((e.currentTarget as HTMLSelectElement).value) })}>
              {#each allZones as z}<option value={z.id}>{z.place} ({z.id})</option>{/each}
            </select>
          </label>
          {#if ev.toZone !== null}
            <button onclick={() => switchZone(ev.toZone!)}>Go there</button>
          {/if}
        {:else if ev.kind === "bg"}
          <label class="field">
            <span>Kind</span>
            <select value={isItem(ev) ? 2 : value(ev, "type")} disabled={readOnly || busy} onchange={(e) => save(ev, { type: Number((e.currentTarget as HTMLSelectElement).value) })}>
              <option value={0}>Sign</option>
              <option value={1}>Something to check</option>
              <option value={2}>Hidden item</option>
            </select>
          </label>
          {#if isItem(ev)}
            <label class="field">
              <span>Item</span>
              <select value={value(ev, "scriptId")} disabled={readOnly || busy} onchange={(e) => save(ev, { scriptId: Number((e.currentTarget as HTMLSelectElement).value) })}>
                {#each items as it}<option value={it.index}>{it.name || `Item ${it.index}`}</option>{/each}
              </select>
            </label>
          {:else}
            <label class="field">
              <span>Read from</span>
              <select value={value(ev, "dir")} disabled={readOnly || busy} onchange={(e) => save(ev, { dir: Number((e.currentTarget as HTMLSelectElement).value) }, false)}>
                {#each DIRS as d, i}<option value={i}>{d}</option>{/each}
              </select>
            </label>
          {/if}
        {:else if ev.kind === "pos"}
          <div class="row">
            <label class="field">
              <span>Width</span>
              <input type="number" min="1" max="255" value={ev.w} readonly={readOnly} onchange={(e) => save(ev, { sizeX: Number((e.currentTarget as HTMLInputElement).value) })} />
            </label>
            <label class="field">
              <span>Height</span>
              <input type="number" min="1" max="255" value={ev.h} readonly={readOnly} onchange={(e) => save(ev, { sizeZ: Number((e.currentTarget as HTMLInputElement).value) })} />
            </label>
          </div>
        {/if}

        {/if}
        {#if ptab === "says" && scriptOf(ev)}
          {@const sc = scriptOf(ev)!}
          <section class="does">
            {#if sc.messages.length}
              <h3>Says</h3>
              {#each uniqueMessages(sc) as m (`${m.bank}:${m.id}`)}
                {@const key = `${m.bank}:${m.id}`}
                <div class="line">
                  <textarea
                    rows={Math.min(5, (lines[key] ?? toEdit(m.text)).split("\n").length + 1)}
                    readonly={readOnly}
                    value={lines[key] ?? toEdit(m.text)}
                    oninput={(e) => (lines[key] = (e.currentTarget as HTMLTextAreaElement).value)}
                  ></textarea>
                  {#if lines[key] !== undefined && !readOnly}
                    <div class="row">
                      <button class="primary" disabled={busy} onclick={() => saveLine(m)}>Save</button>
                      <button onclick={() => delete lines[key]}>Discard</button>
                    </div>
                  {/if}
                </div>
              {/each}
            {/if}
            {#if sc.trainers.length || sc.items.length || sc.gifts.length || sc.warps.length || sc.music.length}
              <h3>Does</h3>
              <ul class="effects">
                {#each sc.trainers as t}<li>Battles <strong>{trainerName(t)}</strong></li>{/each}
                {#each sc.items as it}<li>Gives <strong>{itemName(it.item)}</strong>{it.count > 1 ? ` x${it.count}` : ""}</li>{/each}
                {#each sc.gifts as g}<li>Gives <strong>{species[g.species]?.name || `Pokemon ${g.species}`}</strong>, level {g.level}</li>{/each}
                {#each sc.warps as w}<li>Sends you to <button class="link" onclick={() => switchZone(w.zone)}>{zoneName(w.zone)}</button></li>{/each}
                {#each sc.music as mu}<li>Plays {musicName(mu)}</li>{/each}
              </ul>
            {/if}
            {#if !sc.complete}<p class="muted small">Some of this script could not be read.</p>{/if}
          </section>
        {/if}

        {#if ptab === "values"}
        <div class="raw">
          <div class="fields">
            {#each ev.values as v (v.key)}
              <label>
                <span class="muted">{v.label}</span>
                <input
                  type="number"
                  min={v.min}
                  max={v.max}
                  value={edits[v.key] ?? v.value}
                  readonly={readOnly || v.readonly}
                  oninput={(e) => (edits[v.key] = Number((e.currentTarget as HTMLInputElement).value))}
                />
              </label>
            {/each}
          </div>
          {#if !readOnly}
            <button class="primary" disabled={busy || !Object.keys(edits).length} onclick={() => save(ev, edits)}>Save</button>
          {/if}
        </div>
        {/if}

        {#if ptab === "trainer" && ev.trainer !== null}
          <div class="embed">
            {#key ev.trainer}
              <TableEditor tables={["trainers", "trainerParties"]} single start={ev.trainer} {readOnly} version={0} {onSaved} />
            {/key}
          </div>
        {/if}
      {:else if sel && map}
        <div class="head">
          <div class="title">
            <strong>{sel.w * sel.h === 1 ? `Tile ${sel.x}, ${sel.z}` : `${sel.w} x ${sel.h} tiles`}</strong>
            <span class="muted">{selKinds.map((k) => kindName(k.terrain)).slice(0, 3).join(", ")}</span>
          </div>
        </div>
        {#if !readOnly}
          <label class="field">
            <span>Kind</span>
            <select
              value={selKinds.length === 1 ? selKinds[0].terrain : -1}
              onchange={(e) => {
                const t = Number((e.currentTarget as HTMLSelectElement).value);
                applySelection((i, x, z) => [x, z, t, flags[i]]);
              }}
            >
              {#if selKinds.length !== 1}<option value={-1} disabled>Mixed</option>{/if}
              {#each commonKinds as t}<option value={t}>{kindName(t)}</option>{/each}
            </select>
          </label>
          <div class="row">
            <button onclick={() => applySelection((i, x, z) => [x, z, terrain[i], flags[i] | FLAG_BLOCKED])}>Block</button>
            <button onclick={() => applySelection((i, x, z) => [x, z, terrain[i], flags[i] & ~FLAG_BLOCKED])}>Unblock</button>
            <button onclick={() => applySelection((_, x, z) => [x, z, brush.terrain, brush.flags])}>Fill with paint</button>
          </div>
          <p class="muted small">Drag the selection to move it. Hold Alt to copy. Ctrl+C and Ctrl+V copy and paste.</p>
        {/if}
      {:else if map}
        <div class="head">
          <div class="title"><strong>{map.zone.place}</strong><span class="muted">Map {map.zone.id}, {map.width} x {map.height} tiles</span></div>
        </div>
        <div class="ptabs" role="tablist">
          {#each [["main", "On this map"], ["music", "Music"], ["wild", "Wild Pokemon"]] as [id, label]}
            <button role="tab" aria-selected={ptab === id} class:primary={ptab === id} onclick={() => (ptab = id)}>{label}</button>
          {/each}
        </div>
        {#if ptab === "main"}
        <section>
          {#each counts as [label, list]}
            {#if list.length}
              <details>
                <summary>{label} <span class="muted">{list.length}</span></summary>
                <ul class="list">
                  {#each list as ev (ev.kind + ev.index)}
                    <li><button class="row-item" onclick={() => ((picked = ev), (tool = "select"), play("select"))}>{eventLabel(ev)}</button></li>
                  {/each}
                </ul>
              </details>
            {/if}
          {/each}
        </section>
        {:else if ptab === "music"}
        <section>
          {#each SEASONS as season, i}
            <div class="music">
              <span>{season}</span>
              <select value={map.zone.music[i]} disabled={readOnly} onchange={(e) => saveMusic(i, Number((e.currentTarget as HTMLSelectElement).value))}>
                {#each music as m}<option value={m.index}>{m.name}</option>{/each}
                {#if !music.some((m) => m.index === map!.zone.music[i])}<option value={map.zone.music[i]}>{musicName(map.zone.music[i])}</option>{/if}
              </select>
              <button class:primary={playing === i} onclick={() => toggleMusic(i)}>{playing === i ? "Stop" : "Play"}</button>
            </div>
          {/each}
        </section>
        {:else if ptab === "wild"}
          {#if map.zone.encounters !== null}
            {#key map.zone.encounters}
              <WildEditor index={map.zone.encounters} {readOnly} {onSaved} />
            {/key}
          {:else}
            <p class="muted">No wild Pokemon on this map.</p>
          {/if}
        {/if}
      {/if}
    </aside>
  </div>

</div>

<style>
  /* Bars keep their size; the body takes the rest, whatever is shown. */
  .zone-editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.35rem 0.9rem;
    border-bottom: 2px solid var(--outline);
  }
  .bar h2 {
    margin: 0;
    font-size: 1.05rem;
  }
  .bar button {
    padding: 0.2rem 0.7rem;
  }
  .spacer {
    flex: 1;
  }
  .seg {
    display: flex;
    gap: 0.2rem;
  }
  .body {
    flex: 1 1 0;
    display: grid;
    grid-template-columns: 11rem minmax(0, 1fr) 24rem;
    min-height: 0;
  }
  .tools {
    border-right: 2px solid var(--outline);
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.5rem;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .tools h3 {
    margin: 0.6rem 0 0.1rem;
    font-size: 0.8rem;
    color: var(--muted);
  }
  .tool {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.3rem 0.6rem;
    text-align: left;
  }
  .tool kbd {
    margin-left: auto;
    font-size: 0.7rem;
    opacity: 0.6;
  }
  .brush {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .brush-name {
    font-size: 0.85rem;
    font-weight: 600;
  }
  .chip {
    display: inline-block;
    flex: none;
    width: 0.9rem;
    height: 0.9rem;
    border: 2px solid var(--outline);
    border-radius: 3px;
  }
  .chip.big {
    width: 1.6rem;
    height: 1.6rem;
  }
  .kinds {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .kind {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.2rem 0.4rem;
    box-shadow: none;
    background: transparent;
    border: 2px solid transparent;
    text-align: left;
    font-size: 0.8rem;
  }
  .kind.on {
    border-color: var(--gold);
  }
  .kind-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
  }
  .stage {
    position: relative;
    overflow: hidden;
    min-width: 0;
    min-height: 0;
    background: var(--stage);
    cursor: default;
    touch-action: none;
    user-select: none;
  }
  .stage.panning {
    cursor: grab;
  }
  .world {
    position: absolute;
    left: 0;
    top: 0;
    transform-origin: 0 0;
  }
  .render,
  .tiles {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
    pointer-events: none;
  }
  .render.sharp {
    opacity: 0;
    transition: opacity 0.2s;
  }
  .render.sharp:global(.ready) {
    opacity: 1;
  }
  .render.hidden {
    visibility: hidden;
  }
  .tiles {
    transition: opacity 0.15s;
  }
  .grid {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background-image: linear-gradient(to right, rgba(0, 0, 0, 0.35) 1px, transparent 1px),
      linear-gradient(to bottom, rgba(0, 0, 0, 0.35) 1px, transparent 1px),
      linear-gradient(to right, rgba(0, 0, 0, 0.12) 1px, transparent 1px),
      linear-gradient(to bottom, rgba(0, 0, 0, 0.12) 1px, transparent 1px);
    background-size:
      var(--c) var(--c),
      var(--c) var(--c),
      var(--s) var(--s),
      var(--s) var(--s);
  }
  .ev {
    position: absolute;
    display: grid;
    place-items: center;
    box-sizing: border-box;
    border: 2px solid rgba(0, 0, 0, 0.7);
    border-radius: 4px;
    color: #fff;
    font-weight: 800;
    line-height: 1;
    text-shadow: 0 1px 1px #0008;
    cursor: grab;
    min-width: 6px;
    min-height: 6px;
  }
  .ev.sprite {
    background: transparent !important;
    border-color: transparent;
    overflow: visible;
  }
  .person {
    position: absolute;
    bottom: 0;
    image-rendering: pixelated;
    pointer-events: none;
  }
  .ev.sprite.on {
    outline-offset: -1px;
  }
  .flag {
    position: absolute;
    top: -60%;
    right: -30%;
    min-width: 0.9em;
    padding: 0 0.15em;
    border-radius: 0.5em;
    background: #ff4d4d;
    border: 1px solid #0008;
    font-size: 0.8em;
    text-align: center;
  }
  .badge.pic {
    background: var(--bg);
    image-rendering: pixelated;
  }
  .look {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.2rem 0.5rem;
    text-align: left;
  }
  .look img,
  .sprite-pick img {
    width: 32px;
    height: 32px;
    image-rendering: pixelated;
  }
  .sprites {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(40px, 1fr));
    gap: 2px;
    max-height: 16rem;
    overflow-y: auto;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    padding: 2px;
  }
  .sprite-pick {
    padding: 2px;
    box-shadow: none;
    background: transparent;
    border: 2px solid transparent;
  }
  .sprite-pick.on {
    border-color: var(--gold);
  }
  .zoomer {
    display: flex;
    align-items: center;
  }
  .zoomer button {
    padding: 0.2rem 0.5rem;
  }
  .zoomer .zoom {
    min-width: 3.5rem;
    font-variant-numeric: tabular-nums;
  }
  .banner {
    position: absolute;
    top: 0.6rem;
    left: 50%;
    transform: translateX(-50%);
    padding: 0.3rem 0.8rem;
    font-size: 0.85rem;
    pointer-events: none;
    z-index: 20;
  }
  .preview {
    position: absolute;
    right: 0.75rem;
    bottom: 0.75rem;
    margin: 0;
    padding: 0.3rem;
    z-index: 20;
    pointer-events: none;
  }
  .preview img {
    display: block;
    width: 384px;
    height: 288px;
    image-rendering: pixelated;
  }
  .preview figcaption {
    font-size: 0.75rem;
    padding: 0.15rem 0.2rem 0;
  }
  .does h3 {
    margin: 0.3rem 0;
    font-size: 0.9rem;
  }
  .line {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    margin-bottom: 0.4rem;
  }
  .line textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
    font-size: 0.85rem;
  }
  .effects {
    margin: 0;
    padding-left: 1.1rem;
    font-size: 0.85rem;
  }
  .link {
    padding: 0;
    background: none;
    box-shadow: none;
    border: none;
    color: var(--accent, inherit);
    text-decoration: underline;
    cursor: pointer;
  }
  .prop {
    position: absolute;
    box-sizing: border-box;
    border: 2px solid #fff;
    outline: 2px solid #0009;
    border-radius: 2px;
    background: rgba(255, 140, 0, 0.55);
    transform: rotate(45deg) scale(0.7);
    cursor: grab;
    z-index: 1;
  }
  .prop.on {
    background: var(--gold);
    outline-color: var(--outline);
    z-index: 4;
  }
  .prop.passive {
    pointer-events: none;
  }
  .prop-badge,
  .dot.prop {
    background: rgba(255, 140, 0, 0.85);
  }
  .ev.passive {
    pointer-events: none;
    opacity: 0.7;
  }
  .ev.npc,
  .dot.npc,
  .badge.npc {
    background: #3aa0ff;
    border-radius: 50%;
  }
  .ev.trainer,
  .badge.trainer {
    background: #ff4d4d;
  }
  .ev.warp,
  .dot.warp,
  .badge.warp {
    background: rgba(140, 90, 255, 0.85);
  }
  .ev.pos,
  .dot.pos,
  .badge.pos {
    background: rgba(255, 200, 40, 0.35);
    border-style: dashed;
    border-color: rgba(200, 140, 0, 0.9);
  }
  .ev.bg,
  .dot.bg,
  .badge.bg {
    background: #2fcf8f;
  }
  .ev.item,
  .dot.item,
  .badge.item {
    background: #f0a020;
  }
  .ev:hover {
    filter: brightness(1.15);
    z-index: 3;
  }
  .ev.on {
    outline: 3px solid var(--gold);
    outline-offset: 1px;
    z-index: 4;
  }
  .handle {
    position: absolute;
    right: -6px;
    bottom: -6px;
    width: 12px;
    height: 12px;
    background: var(--gold);
    border: 2px solid var(--outline);
    border-radius: 3px;
    cursor: nwse-resize;
  }
  .dot {
    display: inline-block;
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 3px;
  }
  .sel {
    position: absolute;
    pointer-events: none;
    border: 2px dashed var(--gold);
    background: color-mix(in srgb, var(--gold) 18%, transparent);
    z-index: 2;
  }
  .sel.moving {
    background: color-mix(in srgb, var(--gold) 35%, transparent);
  }
  .sel.box {
    border-style: solid;
  }
  .cursor {
    position: absolute;
    pointer-events: none;
    border: 2px solid var(--gold);
    z-index: 2;
  }
  .tip {
    position: absolute;
    pointer-events: none;
    padding: 0.2rem 0.5rem;
    font-size: 0.8rem;
    white-space: nowrap;
    z-index: 10;
  }
  .panel {
    border-left: 2px solid var(--outline);
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .title {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .title strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    display: grid;
    place-items: center;
    flex: none;
    width: 2rem;
    height: 2rem;
    border-radius: 6px;
    border: 2px solid var(--outline);
    color: #fff;
    font-weight: 800;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.85rem;
    min-width: 0;
    flex: 1;
  }
  .field > span {
    color: var(--muted);
  }
  .field select,
  .field input {
    width: 100%;
  }
  section details summary {
    cursor: pointer;
    font-size: 0.85rem;
  }
  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.4rem;
    margin: 0.5rem 0;
  }
  .fields label {
    display: flex;
    flex-direction: column;
    font-size: 0.75rem;
    min-width: 0;
  }
  .fields input {
    width: 100%;
  }
  section h3 {
    margin: 0.2rem 0 0.4rem;
    font-size: 0.9rem;
  }
  .list {
    list-style: none;
    margin: 0.2rem 0 0.4rem;
    padding: 0;
  }
  .list .row-item {
    width: 100%;
    text-align: left;
  }
  .music {
    display: grid;
    grid-template-columns: 4.5rem minmax(0, 1fr) 4rem;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.3rem;
    font-size: 0.85rem;
  }
  .music button {
    padding: 0.2rem 0.6rem;
  }
  .ptabs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    border-bottom: 2px solid var(--outline);
    padding-bottom: 0.5rem;
  }
  .ptabs button {
    padding: 0.2rem 0.65rem;
    font-size: 0.85rem;
  }
  .embed {
    min-width: 0;
    border-top: 2px solid var(--outline);
    padding-top: 0.6rem;
  }
  .grow {
    display: flex;
    flex-direction: column;
    flex: 1 0 auto;
  }
  .small {
    font-size: 0.8rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
