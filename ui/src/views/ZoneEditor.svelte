<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { asset, sounds, type SoundInfo } from "../lib/backend";
  import { tableEntries, type Entry, type Value } from "../lib/tables";
  import { play } from "../lib/sfx";

  // One zone, tile by tile: a tile layer to paint and an event layer to
  // place people, warps, triggers and signs on.
  let {
    zone,
    zones = [],
    readOnly,
    onBack,
    onOpen,
    onSaved,
  }: {
    zone: number;
    zones?: { id: number; label: string }[];
    readOnly: boolean;
    onBack: () => void;
    onOpen: (category: string, index: number) => void;
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
  interface ZoneMap {
    zone: { id: number; place: string; encounters: number | null; music: number[] };
    width: number;
    height: number;
    terrain: number[];
    flags: number[];
    events: MapEvent[];
  }
  interface Brush {
    terrain: number;
    flags: number;
  }
  type Layer = "tiles" | "events";
  type Tool = "pencil" | "rect" | "fill" | "pick" | "select" | Kind;
  // One undo step: the tiles a stroke changed and what they were before.
  type Step = { i: number; terrain: number; flags: number }[];

  const KIND_LABEL: Record<Kind, string> = {
    npc: "Person",
    warp: "Warp",
    pos: "Trigger",
    bg: "Sign",
  };
  const TILE_TOOLS: [Tool, string, string][] = [
    ["pencil", "Pencil", "P"],
    ["rect", "Rectangle", "R"],
    ["fill", "Fill", "F"],
    ["pick", "Pick", "I"],
  ];
  const EVENT_TOOLS: [Tool, string, string][] = [
    ["select", "Select", "S"],
    ["npc", "Person", "1"],
    ["warp", "Warp", "2"],
    ["pos", "Trigger", "3"],
    ["bg", "Sign", "4"],
  ];

  // svelte-ignore state_referenced_locally
  let current = $state(zone);
  let map = $state<ZoneMap | null>(null);
  let tab = $state<"map" | "music" | "wild">("map");
  let layer = $state<Layer>("tiles");
  let tool = $state<Tool>("pencil");
  let brush = $state<Brush>({ terrain: 0, flags: 1 });
  let selected = $state<MapEvent | null>(null);
  let edits = $state<Record<string, number>>({});
  let scale = $state(12);
  let grid = $state(true);
  let hover = $state<{ x: number; z: number } | null>(null);
  let rect = $state<{ x0: number; z0: number; x1: number; z1: number } | null>(null);
  let undo = $state<Step[]>([]);
  let trainers = $state<Entry[]>([]);
  let music = $state<SoundInfo[]>([]);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let canvas = $state<HTMLCanvasElement | null>(null);
  let stage = $state<HTMLDivElement | null>(null);
  let audio: HTMLAudioElement | null = null;
  // The tiles of the stroke in progress, by index, with their old values.
  let stroke: Map<number, { terrain: number; flags: number }> | null = null;
  let drag: { ev: MapEvent; x: number; z: number; dx: number; dz: number } | null = null;

  async function load() {
    error = null;
    try {
      const next = await invoke<ZoneMap>("zone_map", { zone: current });
      const keep = selected;
      map = next;
      selected = keep ? (next.events.find((e) => e.kind === keep.kind && e.index === keep.index) ?? null) : null;
      edits = {};
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    load().then(fit);
    tableEntries("trainers").then((t) => (trainers = t)).catch(() => {});
    sounds().then((s) => (music = s.filter((x) => x.group === "music"))).catch(() => {});
    return () => audio?.pause();
  });

  // The largest tile size that shows the whole map in the pane.
  function fit() {
    if (!stage || !map) return;
    const w = stage.clientWidth - 32;
    const h = stage.clientHeight - 32;
    scale = Math.max(4, Math.min(24, Math.floor(Math.min(w / map.width, h / map.height))));
  }

  // A colour per terrain id; tiles you cannot walk on are darker.
  function color(terrain: number, flags: number): string {
    if (terrain < 0) return "#00000000";
    const hue = (terrain * 47) % 360;
    return `hsl(${hue} ${terrain === 0 ? 0 : 45}% ${flags & 1 ? 62 : 32}%)`;
  }

  // The terrain and flag pairs the map uses, most common first.
  const palette = $derived.by(() => {
    if (!map) return [] as (Brush & { count: number })[];
    const counts = new Map<number, number>();
    for (let i = 0; i < map.terrain.length; i++) {
      if (map.terrain[i] < 0) continue;
      const key = map.terrain[i] * 0x10000 + map.flags[i];
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return [...counts]
      .map(([key, count]) => ({ terrain: Math.floor(key / 0x10000), flags: key % 0x10000, count }))
      .sort((a, b) => b.count - a.count);
  });

  $effect(() => {
    if (!canvas || !map) return;
    const ctx = canvas.getContext("2d")!;
    const { width, height, terrain, flags } = map;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    for (let z = 0; z < height; z++)
      for (let x = 0; x < width; x++) {
        const i = z * width + x;
        if (terrain[i] < 0) continue;
        ctx.fillStyle = color(terrain[i], flags[i]);
        ctx.fillRect(x * scale, z * scale, scale, scale);
      }
    if (grid && scale >= 6) {
      ctx.strokeStyle = "rgba(0,0,0,0.15)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let x = 0; x <= width; x++) {
        ctx.moveTo(x * scale + 0.5, 0);
        ctx.lineTo(x * scale + 0.5, height * scale);
      }
      for (let z = 0; z <= height; z++) {
        ctx.moveTo(0, z * scale + 0.5);
        ctx.lineTo(width * scale, z * scale + 0.5);
      }
      ctx.stroke();
      // Chunk borders every 32 tiles.
      ctx.strokeStyle = "rgba(0,0,0,0.4)";
      ctx.beginPath();
      for (let x = 0; x <= width; x += 32) {
        ctx.moveTo(x * scale + 0.5, 0);
        ctx.lineTo(x * scale + 0.5, height * scale);
      }
      for (let z = 0; z <= height; z += 32) {
        ctx.moveTo(0, z * scale + 0.5);
        ctx.lineTo(width * scale, z * scale + 0.5);
      }
      ctx.stroke();
    }
  });

  function at(e: PointerEvent): { x: number; z: number } {
    const r = canvas!.getBoundingClientRect();
    return { x: Math.floor((e.clientX - r.left) / scale), z: Math.floor((e.clientY - r.top) / scale) };
  }
  const inside = (p: { x: number; z: number }) => !!map && p.x >= 0 && p.z >= 0 && p.x < map.width && p.z < map.height;

  // Paints one tile locally and records it in the current stroke.
  function touch(i: number) {
    if (!map || !stroke || map.terrain[i] < 0) return;
    if (map.terrain[i] === brush.terrain && map.flags[i] === brush.flags) return;
    if (!stroke.has(i)) stroke.set(i, { terrain: map.terrain[i], flags: map.flags[i] });
    map.terrain[i] = brush.terrain;
    map.flags[i] = brush.flags;
  }

  function flood(p: { x: number; z: number }) {
    const m = map!;
    const start = p.z * m.width + p.x;
    const from = [m.terrain[start], m.flags[start]];
    if (from[0] === brush.terrain && from[1] === brush.flags) return;
    const seen = new Set<number>([start]);
    const todo = [start];
    while (todo.length) {
      const i = todo.pop()!;
      touch(i);
      const x = i % m.width;
      for (const j of [i - m.width, i + m.width, x > 0 ? i - 1 : -1, x < m.width - 1 ? i + 1 : -1]) {
        if (j < 0 || j >= m.terrain.length || seen.has(j)) continue;
        if (m.terrain[j] !== from[0] || m.flags[j] !== from[1]) continue;
        seen.add(j);
        todo.push(j);
      }
    }
  }

  function onMapDown(e: PointerEvent) {
    if (!map || e.button !== 0) return;
    const p = at(e);
    if (!inside(p)) return;
    const i = p.z * map.width + p.x;
    if (layer === "events") {
      if (tool === "select") {
        selected = null;
        return;
      }
      if (!readOnly) addEvent(tool as Kind, p);
      return;
    }
    if (tool === "pick" || e.altKey) {
      if (map.terrain[i] >= 0) brush = { terrain: map.terrain[i], flags: map.flags[i] };
      play("select");
      return;
    }
    if (readOnly) return;
    stroke = new Map();
    if (tool === "fill") {
      flood(p);
      commit();
      return;
    }
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    if (tool === "rect") rect = { x0: p.x, z0: p.z, x1: p.x, z1: p.z };
    else touch(i);
  }

  function onMapMove(e: PointerEvent) {
    if (!map) return;
    const p = at(e);
    hover = inside(p) ? p : null;
    if (!stroke || !inside(p)) return;
    if (rect) {
      rect.x1 = p.x;
      rect.z1 = p.z;
    } else if (tool === "pencil") touch(p.z * map.width + p.x);
  }

  function onMapUp() {
    if (!stroke || !map) return;
    if (rect) {
      const [xa, xb] = [Math.min(rect.x0, rect.x1), Math.max(rect.x0, rect.x1)];
      const [za, zb] = [Math.min(rect.z0, rect.z1), Math.max(rect.z0, rect.z1)];
      for (let z = za; z <= zb; z++) for (let x = xa; x <= xb; x++) touch(z * map.width + x);
      rect = null;
    }
    commit();
  }

  // Writes the stroke's tiles in one call.
  async function commit() {
    const done = stroke;
    stroke = null;
    if (!done || !done.size || !map) return;
    const width = map.width;
    const tiles = [...done.keys()].map((i) => [i % width, Math.floor(i / width)]);
    undo = [...undo.slice(-49), [...done].map(([i, old]) => ({ i, ...old }))];
    try {
      await invoke("set_tiles", { zone: current, tiles, terrain: brush.terrain, flags: brush.flags });
      play("select");
      onSaved();
    } catch (err) {
      error = String(err);
      await load();
    }
  }

  async function undoStep() {
    const step = undo.at(-1);
    if (!step || !map || readOnly) return;
    undo = undo.slice(0, -1);
    const width = map.width;
    // One call per old value.
    const groups = new Map<number, number[][]>();
    for (const { i, terrain, flags } of step) {
      map.terrain[i] = terrain;
      map.flags[i] = flags;
      const key = terrain * 0x10000 + flags;
      groups.set(key, [...(groups.get(key) ?? []), [i % width, Math.floor(i / width)]]);
    }
    try {
      for (const [key, tiles] of groups)
        await invoke("set_tiles", { zone: current, tiles, terrain: Math.floor(key / 0x10000), flags: key % 0x10000 });
      play("cancel");
      onSaved();
    } catch (err) {
      error = String(err);
      await load();
    }
  }

  async function addEvent(kind: Kind, p: { x: number; z: number }) {
    busy = true;
    try {
      const index = await invoke<number>("add_event", { zone: current, kind, x: p.x, z: p.z });
      play("decide");
      selected = null;
      await load();
      selected = map!.events.find((ev) => ev.kind === kind && ev.index === index) ?? null;
      tool = "select";
      onSaved();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function pickEvent(ev: MapEvent, e: PointerEvent) {
    if (layer !== "events" || tool !== "select") return;
    e.stopPropagation();
    play("select");
    selected = ev;
    edits = {};
    if (!readOnly && (ev.kind === "npc" || ev.kind === "bg")) {
      const p = at(e);
      drag = { ev, x: ev.x, z: ev.z, dx: p.x - ev.x, dz: p.z - ev.z };
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    }
  }

  function onDragMove(e: PointerEvent) {
    if (!drag || !map) return;
    const p = at(e);
    drag.ev.x = Math.max(0, Math.min(map.width - 1, p.x - drag.dx));
    drag.ev.z = Math.max(0, Math.min(map.height - 1, p.z - drag.dz));
  }

  // Saved positions are matrix tiles: move them by the drag.
  async function onDragEnd() {
    if (!drag) return;
    const { ev, x, z } = drag;
    drag = null;
    const [kx, kz] = ev.kind === "npc" ? ["gridX", "gridZ"] : ["x", "z"];
    const vx = ev.values.find((v) => v.key === kx);
    const vz = ev.values.find((v) => v.key === kz);
    if (!vx || !vz || (ev.x === x && ev.z === z)) return;
    await save(ev, { [kx]: vx.value + ev.x - x, [kz]: vz.value + ev.z - z });
  }

  async function save(ev: MapEvent, values: Record<string, number>) {
    busy = true;
    try {
      await invoke("save_event", { zone: current, kind: ev.kind, index: ev.index, values });
      play("save");
      await load();
      onSaved();
    } catch (err) {
      error = String(err);
      play("cancel");
      await load();
    } finally {
      busy = false;
    }
  }

  async function remove(ev: MapEvent) {
    busy = true;
    try {
      await invoke("remove_event", { zone: current, kind: ev.kind, index: ev.index });
      play("cancel");
      selected = null;
      await load();
      onSaved();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

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

  function playMusic(seq: number) {
    audio?.pause();
    audio = new Audio(asset(`sound/${seq}?seconds=60`));
    audio.play().catch(() => {});
  }

  function switchZone(id: number) {
    play("select");
    current = id;
    selected = null;
    undo = [];
    load().then(fit);
  }

  function setLayer(l: Layer) {
    layer = l;
    tool = l === "tiles" ? "pencil" : "select";
    if (l === "tiles") selected = null;
    play("select");
  }

  function onKey(e: KeyboardEvent) {
    if (tab !== "map" || (e.target as HTMLElement).closest("input, select, textarea")) return;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
      e.preventDefault();
      undoStep();
      return;
    }
    if (e.key === "Escape") selected = null;
    if (e.key === "Delete" && selected && !readOnly) remove(selected);
    const k = e.key.toUpperCase();
    const tile = TILE_TOOLS.find((t) => t[2] === k);
    const ev = EVENT_TOOLS.find((t) => t[2] === k);
    if (tile) (layer = "tiles"), (tool = tile[0]);
    else if (ev) (layer = "events"), (tool = ev[0]);
    else if (k === "G") grid = !grid;
  }

  const trainerName = (id: number) => trainers[id]?.name ?? `Trainer ${id}`;
  const musicName = (seq: number) => music.find((m) => m.index === seq)?.name ?? `#${seq}`;
  const eventLabel = (ev: MapEvent) =>
    ev.trainer !== null ? trainerName(ev.trainer) : `${KIND_LABEL[ev.kind]} ${ev.index}`;
  const hoverIndex = $derived(hover && map ? hover.z * map.width + hover.x : -1);
  const rectBox = $derived(
    rect
      ? {
          x: Math.min(rect.x0, rect.x1),
          z: Math.min(rect.z0, rect.z1),
          w: Math.abs(rect.x1 - rect.x0) + 1,
          h: Math.abs(rect.z1 - rect.z0) + 1,
        }
      : null,
  );
</script>

<svelte:window onkeydown={onKey} />

<div class="zone-editor">
  <header class="bar">
    <button onclick={() => (play("cancel"), onBack())}>World map</button>
    <h2>{map?.zone.place ?? ""}</h2>
    {#if zones.length > 1}
      <select value={current} onchange={(e) => switchZone(Number((e.currentTarget as HTMLSelectElement).value))} aria-label="Map">
        {#each zones as z}<option value={z.id}>{z.label}</option>{/each}
      </select>
    {/if}
    <div class="tabs" role="tablist">
      {#each [["map", "Map"], ["music", "Music"], ["wild", "Wild Pokemon"]] as [id, label]}
        <button role="tab" aria-selected={tab === id} class:primary={tab === id} onclick={() => ((tab = id as typeof tab), play("select"))}>{label}</button>
      {/each}
    </div>
  </header>

  {#if error}<p class="bad pad">{error}</p>{/if}

  {#if tab === "map"}
    <div class="body">
      <aside class="palette">
        <div class="layers" role="radiogroup" aria-label="Layer">
          <button class:primary={layer === "tiles"} onclick={() => setLayer("tiles")}>Tiles</button>
          <button class:primary={layer === "events"} onclick={() => setLayer("events")}>Events</button>
        </div>

        <div class="tools">
          {#each layer === "tiles" ? TILE_TOOLS : EVENT_TOOLS as [t, label, key]}
            <button
              class:primary={tool === t}
              disabled={readOnly && t !== "pick" && t !== "select"}
              title="{label} ({key})"
              onclick={() => ((tool = t), play("select"))}
            >
              {#if layer === "events" && t !== "select"}<i class="dot {t}"></i>{/if}{label}
            </button>
          {/each}
        </div>

        {#if layer === "tiles"}
          <div class="brush">
            <span class="swatch big" style="background: {color(brush.terrain, brush.flags)}"></span>
            <label><span class="muted">Terrain</span><input type="number" min="0" max="65535" bind:value={brush.terrain} /></label>
            <label><span class="muted">Flags</span><input type="number" min="0" max="32767" bind:value={brush.flags} /></label>
            <label class="check"><input type="checkbox" checked={(brush.flags & 1) === 1} onchange={() => (brush.flags ^= 1)} />Walkable</label>
          </div>
          <div class="swatches">
            {#each palette as p (p.terrain * 0x10000 + p.flags)}
              <button
                class="swatch"
                class:on={p.terrain === brush.terrain && p.flags === brush.flags}
                style="background: {color(p.terrain, p.flags)}"
                title="Terrain {p.terrain}, flags {p.flags} ({p.count} tiles)"
                onclick={() => ((brush = { terrain: p.terrain, flags: p.flags }), play("select"))}
              >{p.terrain}</button>
            {/each}
          </div>
        {:else if map}
          <ul class="event-list">
            {#each map.events as ev (ev.kind + ev.index)}
              <li>
                <button class="row-item" class:on={selected === ev} onclick={() => ((selected = ev), (edits = {}), (tool = "select"), play("select"))}>
                  <i class="dot {ev.kind}" class:trainer={ev.trainer !== null}></i>
                  <span class="label">{eventLabel(ev)}</span>
                  <span class="muted">{ev.x}, {ev.z}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </aside>

      <div
        class="stage"
        bind:this={stage}
        onwheel={(e) => {
          if (!e.ctrlKey) return;
          e.preventDefault();
          scale = Math.max(3, Math.min(32, scale + (e.deltaY < 0 ? 1 : -1)));
        }}
      >
        {#if map}
          <div class="canvas" class:events={layer === "events"} style="width: {map.width * scale}px; height: {map.height * scale}px">
            <canvas
              bind:this={canvas}
              width={map.width * scale}
              height={map.height * scale}
              onpointerdown={onMapDown}
              onpointermove={onMapMove}
              onpointerup={onMapUp}
              onpointerleave={() => (hover = null)}
            ></canvas>
            {#each map.events as ev (ev.kind + ev.index)}
              {#if ev.x >= 0 && ev.z >= 0}
                <button
                  class="ev {ev.kind}"
                  class:trainer={ev.trainer !== null}
                  class:on={selected === ev}
                  style="left: {ev.x * scale}px; top: {ev.z * scale}px; width: {ev.w * scale}px; height: {ev.h * scale}px"
                  title={eventLabel(ev)}
                  aria-label={eventLabel(ev)}
                  onpointerdown={(e) => pickEvent(ev, e)}
                  onpointermove={onDragMove}
                  onpointerup={onDragEnd}
                ></button>
              {/if}
            {/each}
            {#if rectBox}
              <div class="rect" style="left: {rectBox.x * scale}px; top: {rectBox.z * scale}px; width: {rectBox.w * scale}px; height: {rectBox.h * scale}px"></div>
            {/if}
            {#if hover && layer === "tiles" && !rect}
              <div class="cursor" style="left: {hover.x * scale}px; top: {hover.z * scale}px; width: {scale}px; height: {scale}px"></div>
            {/if}
          </div>
        {:else}
          <p class="muted pad">Loading...</p>
        {/if}
      </div>

      <aside class="inspector">
        {#if selected}
          <div class="head">
            <i class="dot {selected.kind}" class:trainer={selected.trainer !== null}></i>
            <strong>{eventLabel(selected)}</strong>
            <span class="tag">{KIND_LABEL[selected.kind]}</span>
          </div>
          {#if selected.trainer !== null}
            <button class="primary" onclick={() => onOpen("trainers", selected!.trainer!)}>Edit trainer</button>
          {/if}
          {#if selected.toZone !== null}
            <button onclick={() => switchZone(selected!.toZone!)}>Go to map {selected.toZone}</button>
          {/if}
          <div class="fields">
            {#each selected.values as v (v.key)}
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
            <div class="actions">
              <button class="primary" disabled={busy || !Object.keys(edits).length} onclick={() => save(selected!, edits)}>Save</button>
              <button disabled={busy} onclick={() => remove(selected!)}>Delete</button>
            </div>
          {/if}
        {:else if map}
          <div class="head"><strong>Map {map.zone.id}</strong></div>
          <dl>
            <dt>Size</dt><dd>{map.width} x {map.height}</dd>
            <dt>People</dt><dd>{map.events.filter((e) => e.kind === "npc").length}</dd>
            <dt>Trainers</dt><dd>{map.events.filter((e) => e.trainer !== null).length}</dd>
            <dt>Warps</dt><dd>{map.events.filter((e) => e.kind === "warp").length}</dd>
            <dt>Triggers</dt><dd>{map.events.filter((e) => e.kind === "pos").length}</dd>
            <dt>Signs</dt><dd>{map.events.filter((e) => e.kind === "bg").length}</dd>
          </dl>
        {/if}
      </aside>
    </div>

    <footer class="status">
      {#if hover && map && hoverIndex >= 0}
        <span>{hover.x}, {hover.z}</span>
        <span>Terrain {map.terrain[hoverIndex]}</span>
        <span>Flags {map.flags[hoverIndex]}</span>
      {:else}
        <span class="muted">-</span>
      {/if}
      <span class="spacer"></span>
      <button class="quiet" disabled={!undo.length || readOnly} onclick={undoStep} title="Undo (Ctrl+Z)">Undo</button>
      <label class="check"><input type="checkbox" bind:checked={grid} />Grid</label>
      <input type="range" min="3" max="32" bind:value={scale} aria-label="Zoom" />
      <button class="quiet" onclick={fit}>Fit</button>
    </footer>
  {:else if tab === "music" && map}
    <div class="sheet">
      {#each ["Spring", "Summer", "Autumn", "Winter"] as season, i}
        <label class="row">
          <span>{season}</span>
          <select value={map.zone.music[i]} disabled={readOnly} onchange={(e) => saveMusic(i, Number((e.currentTarget as HTMLSelectElement).value))}>
            {#each music as m}<option value={m.index}>{m.name}</option>{/each}
            {#if !music.some((m) => m.index === map!.zone.music[i])}<option value={map.zone.music[i]}>{musicName(map.zone.music[i])}</option>{/if}
          </select>
          <button onclick={() => playMusic(map!.zone.music[i])}>Play</button>
        </label>
      {/each}
    </div>
  {:else if tab === "wild" && map}
    <div class="sheet">
      {#if map.zone.encounters !== null}
        <p>Encounter table {map.zone.encounters}</p>
        <button class="primary" onclick={() => onOpen("encounters", map!.zone.encounters!)}>Edit wild Pokemon</button>
      {:else}
        <p class="muted">No wild Pokemon on this map.</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .zone-editor {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    height: 100%;
    min-height: 0;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6rem;
    padding: 0.35rem 0.9rem;
    border-bottom: 2px solid var(--outline);
  }
  .bar h2 {
    margin: 0;
    font-size: 1.05rem;
  }
  .tabs {
    display: flex;
    gap: 0.25rem;
    margin-left: auto;
  }
  .bar button,
  .palette button,
  .status button {
    padding: 0.2rem 0.7rem;
  }
  .body {
    display: grid;
    grid-template-columns: 13rem minmax(0, 1fr) 16rem;
    min-height: 0;
  }
  .palette {
    border-right: 2px solid var(--outline);
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.6rem;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .layers {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.25rem;
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }
  .tools button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
  }
  .brush {
    display: grid;
    grid-template-columns: auto 1fr 1fr;
    align-items: end;
    gap: 0.35rem;
  }
  .brush label {
    display: flex;
    flex-direction: column;
    font-size: 0.75rem;
    min-width: 0;
  }
  .brush input[type="number"] {
    width: 100%;
  }
  .brush .check {
    grid-column: 1 / -1;
  }
  .check {
    display: flex;
    flex-direction: row !important;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.8rem;
  }
  .swatches {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(2.2rem, 1fr));
    gap: 0.25rem;
  }
  .swatch {
    height: 2.2rem;
    padding: 0;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    box-shadow: none;
    font-size: 0.7rem;
    font-weight: 700;
    color: #000c;
  }
  .swatch.big {
    display: block;
    width: 2.2rem;
  }
  .swatch.on {
    outline: 3px solid var(--gold);
    outline-offset: 1px;
  }
  .event-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .event-list .row-item {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
  }
  .event-list .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }
  .row-item.on {
    background: color-mix(in srgb, var(--gold) 25%, transparent);
  }
  .stage {
    overflow: auto;
    min-width: 0;
    min-height: 0;
    padding: 1rem;
    background: var(--stage);
  }
  .canvas {
    position: relative;
    margin: auto;
  }
  .canvas canvas {
    display: block;
    cursor: crosshair;
    image-rendering: pixelated;
  }
  .canvas:not(.events) .ev {
    pointer-events: none;
    opacity: 0.6;
  }
  .canvas.events canvas {
    cursor: default;
  }
  .ev {
    position: absolute;
    padding: 0;
    border-radius: 3px;
    box-shadow: none;
    border: 2px solid var(--outline);
    min-width: 6px;
    min-height: 6px;
  }
  .ev.npc,
  .dot.npc {
    background: #3aa0ff;
    border-radius: 50%;
  }
  .ev.npc.trainer,
  .dot.trainer {
    background: #ff4d4d;
    border-radius: 50%;
  }
  .ev.warp,
  .dot.warp {
    background: rgba(140, 90, 255, 0.75);
  }
  .ev.pos,
  .dot.pos {
    background: rgba(255, 200, 40, 0.45);
  }
  .ev.bg,
  .dot.bg {
    background: #2fcf8f;
  }
  .ev.on {
    outline: 3px solid var(--gold);
    outline-offset: 1px;
    z-index: 2;
  }
  .dot {
    display: inline-block;
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 3px;
  }
  .rect,
  .cursor {
    position: absolute;
    pointer-events: none;
    border: 2px solid var(--gold);
    background: color-mix(in srgb, var(--gold) 20%, transparent);
  }
  .inspector {
    border-left: 2px solid var(--outline);
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.25rem 0.75rem;
    margin: 0;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
  }
  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.4rem;
  }
  .fields label {
    display: flex;
    flex-direction: column;
    font-size: 0.8rem;
    min-width: 0;
  }
  .fields input {
    width: 100%;
  }
  .actions {
    display: flex;
    gap: 0.4rem;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0.25rem 0.9rem;
    border-top: 2px solid var(--outline);
    font-size: 0.85rem;
    font-variant-numeric: tabular-nums;
  }
  .status .spacer {
    flex: 1;
  }
  .status input[type="range"] {
    width: 7rem;
  }
  .sheet {
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    overflow-y: auto;
  }
  .row {
    display: grid;
    grid-template-columns: 6rem minmax(0, 22rem) auto;
    align-items: center;
    gap: 0.5rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
