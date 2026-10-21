<script lang="ts">
  import { Pan } from "../lib/pan.svelte";
  import { onMount } from "svelte";
  import {
    animBank,
    cellBank,
    revert,
    savePalette,
    saveSheet,
    sheetData,
    type Layout,
    type Ref,
    type SheetData,
  } from "../lib/backend";
  import { play } from "../lib/sfx";
  import AnimationPlayer from "./AnimationPlayer.svelte";

  let {
    chr,
    pal,
    readOnly,
    changed,
    palettes,
    paletteOffset = 0,
    cells,
    anims,
    rigFrame,
    version = 0,
    dirty = $bindable(false),
    onPalette,
    onSaved,
  }: {
    chr: Ref;
    pal: Ref;
    readOnly: boolean;
    changed: boolean;
    /// The palettes the picture can be shown with: [label, member].
    palettes: [string, number][];
    /// The palette bank the game adds for this picture (Pokemon icons).
    paletteOffset?: number;
    /// The known cell bank (null: a plain picture; undefined: find it).
    cells?: number | null;
    /// The known animation bank (null: none; undefined: find it).
    anims?: number | null;
    /// Edit one frame of a battle rig instead (the sheet is its picture).
    rigFrame?: { cer: Ref; anr: Ref; mcr: Ref; mna?: Ref | null; frame: number } | null;
    /// Changes after a save or a revert: the picture is read again.
    version?: number;
    /// True while there are edits not yet saved.
    dirty?: boolean;
    onPalette: (index: number) => void;
    onSaved: () => void;
  } = $props();

  type Tool = "pencil" | "fill" | "picker";

  // The picture comes from the game's own layout: a sprite's cells
  // assembled as the game draws them, or the plain tiles. Every pixel of
  // the picture maps to one pixel of one tile, so painting edits the tiles.
  let data = $state<SheetData | null>(null);
  let tiles = $state<number[][]>([]);
  let colors = $state<number[]>([]);
  let bankFor = $state<number | null>(null);
  let animFor = $state<number | null>(null);
  let view = $state<"cells" | "grid">("grid");
  let cols = $state(16);
  let zoom = $state(4);
  const pan = new Pan();
  // Ctrl+wheel zooms; added by hand so it can stop the page zoom.
  $effect(() => {
    if (!stage) return;
    const wheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      zoom = Math.max(1, Math.min(maxZoom, zoom + (e.deltaY < 0 ? 1 : -1)));
    };
    stage.addEventListener("wheel", wheel, { passive: false });
    return () => stage!.removeEventListener("wheel", wheel);
  });
  let maxZoom = $state(12);
  // svelte-ignore state_referenced_locally
  let bank = $state(paletteOffset);
  let color = $state(1);
  let tool = $state<Tool>("pencil");
  let tilesDirty = $state(false);
  let paletteDirty = $state(false);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let confirmRevert = $state(false);
  let canvas = $state<HTMLCanvasElement | null>(null);
  let stage = $state<HTMLDivElement | null>(null);

  type Snap = { tiles: number[][]; colors: number[] };
  let undo: Snap[] = [];
  let redo: Snap[] = [];
  let drawing = false;
  let fitted = false;
  // The state before the current stroke; kept only if the stroke changes something.
  let stash: Snap | null = null;
  let last = -1;
  let colorEdit = false;

  $effect(() => {
    dirty = tilesDirty || paletteDirty;
  });
  // A read-only picture can only be inspected.
  $effect(() => {
    if (readOnly) tool = "picker";
  });

  const banks = $derived(data?.bpp === 4 ? Math.max(1, Math.floor(colors.length / 16)) : 1);
  const base = $derived(data?.bpp === 4 ? bank * 16 : 0);
  const swatches = $derived(data?.bpp === 4 ? colors.slice(base, base + 16) : colors);

  async function load(keepEdits: boolean) {
    error = null;
    try {
      const layout: Layout = rigFrame
        ? { kind: "rig", ...rigFrame }
        : view === "cells" && bankFor !== null
          ? { kind: "cells", cer: [chr[0], bankFor] }
          : { kind: "grid", cols };
      const next = await sheetData(chr, pal, layout);
      if (!keepEdits || !data) {
        tiles = next.tiles.map((t) => t.slice());
        colors = next.colors.slice();
      }
      if (!data || data.width !== next.width || data.height !== next.height) fitted = false;
      data = next;
      fit();
    } catch (e) {
      error = String(e);
    }
  }

  // A new rig frame keeps the edits and shows that frame.
  let lastFrame = -1;
  $effect(() => {
    const f = rigFrame?.frame ?? -1;
    if (f !== lastFrame && data && rigFrame) {
      lastFrame = f;
      load(true);
    }
  });

  // A save or a revert: read the picture again, keep zoom, view and tool.
  let lastVersion = -1;
  $effect(() => {
    const v = version;
    if (lastVersion === -1) {
      lastVersion = v;
      return;
    }
    if (v === lastVersion) return;
    lastVersion = v;
    if (!data) return;
    undo = [];
    redo = [];
    tilesDirty = paletteDirty = false;
    load(false);
  });

  onMount(async () => {
    if (rigFrame) {
      lastFrame = rigFrame.frame;
      view = "cells";
      await load(false);
      return;
    }
    if (cells !== undefined) {
      bankFor = cells;
    } else {
      try {
        bankFor = await cellBank(chr);
      } catch {
        bankFor = null;
      }
    }
    view = bankFor !== null ? "cells" : "grid";
    if (bankFor !== null) {
      animFor = anims !== undefined ? anims : await animBank(chr[0], bankFor).catch(() => null);
    }
    await load(false);
  });

  function setView(v: "cells" | "grid") {
    if (v === view) return;
    play("select");
    view = v;
    pan.reset();
    load(true);
  }

  function resetView() {
    fitted = false;
    fit();
    pan.reset();
    play("select");
  }

  function fit() {
    if (!data) return;
    const avail = (stage?.clientWidth ?? 600) - 32;
    maxZoom = Math.max(1, Math.min(16, Math.floor(avail / Math.max(1, data.width))));
    // Start at three quarters of the zoom that fills the pane.
    zoom = fitted ? Math.min(zoom, maxZoom) : Math.max(1, Math.round(maxZoom * 0.75));
    fitted = true;
  }

  $effect(() => {
    if (!stage) return;
    const observer = new ResizeObserver(() => fit());
    observer.observe(stage);
    return () => observer.disconnect();
  });

  const hex = (c: number) => `#${c.toString(16).padStart(6, "0")}`;

  function pixelBase(i: number): number {
    if (!data || data.bpp === 8) return 0;
    return (view === "cells" || rigFrame ? data.banks[i] + paletteOffset : bank) * 16;
  }

  // Redraw whenever the pixels, palette, layout or bank change.
  $effect(() => {
    if (!canvas || !data) return;
    const { width, height, map } = data;
    const ctx = canvas.getContext("2d")!;
    const img = ctx.createImageData(width, height);
    for (let i = 0; i < map.length; i++) {
      const src = map[i];
      if (src < 0) continue;
      const v = tiles[src >> 6][src & 63];
      if (v === 0) continue;
      const c = colors[pixelBase(i) + v] ?? 0xff00ff;
      img.data[i * 4] = c >> 16;
      img.data[i * 4 + 1] = (c >> 8) & 0xff;
      img.data[i * 4 + 2] = c & 0xff;
      img.data[i * 4 + 3] = 255;
    }
    ctx.putImageData(img, 0, 0);
  });

  /// The picture pixel under the pointer, if it maps to a tile.
  function at(e: PointerEvent): number | null {
    if (!data) return null;
    const r = (e.currentTarget as HTMLCanvasElement).getBoundingClientRect();
    const x = Math.floor(((e.clientX - r.left) / r.width) * data.width);
    const y = Math.floor(((e.clientY - r.top) / r.height) * data.height);
    if (x < 0 || y < 0 || x >= data.width || y >= data.height) return null;
    const i = y * data.width + x;
    return data.map[i] < 0 ? null : i;
  }

  function copy(): Snap {
    return { tiles: tiles.map((t) => t.slice()), colors: colors.slice() };
  }

  function snapshot() {
    undo.push(copy());
    if (undo.length > 100) undo.shift();
    redo = [];
  }

  // The first real change of a stroke records the state before it.
  function touch() {
    if (stash) {
      undo.push(stash);
      if (undo.length > 100) undo.shift();
      redo = [];
      stash = null;
    }
    tilesDirty = true;
  }

  function paint(i: number) {
    const src = data!.map[i];
    if (tiles[src >> 6][src & 63] === color) return;
    touch();
    tiles[src >> 6][src & 63] = color;
  }

  // Every pixel between two pointer events, so a fast drag leaves no gaps.
  function line(from: number, to: number) {
    const w = data!.width;
    let x0 = from % w;
    let y0 = Math.floor(from / w);
    const x1 = to % w;
    const y1 = Math.floor(to / w);
    const dx = Math.abs(x1 - x0);
    const dy = -Math.abs(y1 - y0);
    const sx = x0 < x1 ? 1 : -1;
    const sy = y0 < y1 ? 1 : -1;
    let err = dx + dy;
    for (;;) {
      const i = y0 * w + x0;
      if (data!.map[i] >= 0) paint(i);
      if (x0 === x1 && y0 === y1) break;
      const e2 = 2 * err;
      if (e2 >= dy) {
        err += dy;
        x0 += sx;
      }
      if (e2 <= dx) {
        err += dx;
        y0 += sy;
      }
    }
  }

  function fill(start: number) {
    const { width, height, map } = data!;
    const target = tiles[map[start] >> 6][map[start] & 63];
    if (target === color) return;
    touch();
    const stack = [start];
    const seen = new Set<number>();
    while (stack.length) {
      const i = stack.pop()!;
      if (seen.has(i)) continue;
      seen.add(i);
      const src = map[i];
      if (src < 0 || tiles[src >> 6][src & 63] !== target) continue;
      tiles[src >> 6][src & 63] = color;
      const x = i % width;
      const y = Math.floor(i / width);
      if (x > 0) stack.push(i - 1);
      if (x < width - 1) stack.push(i + 1);
      if (y > 0) stack.push(i - width);
      if (y < height - 1) stack.push(i + width);
    }
  }

  function down(e: PointerEvent) {
    const i = at(e);
    if (i === null) return;
    const src = data!.map[i];
    if (e.button === 2 || tool === "picker") {
      color = tiles[src >> 6][src & 63];
      if (view === "cells" && data!.bpp === 4) bank = data!.banks[i] + paletteOffset;
      play("select");
      return;
    }
    if (readOnly || e.button !== 0) return;
    stash = copy();
    if (tool === "fill") {
      fill(i);
      stash = null;
      return;
    }
    drawing = true;
    last = i;
    (e.currentTarget as HTMLCanvasElement).setPointerCapture(e.pointerId);
    paint(i);
  }

  function move(e: PointerEvent) {
    if (!drawing) return;
    const i = at(e);
    if (i === null) return;
    line(last, i);
    last = i;
  }

  function stop() {
    drawing = false;
    stash = null;
  }

  function step(from: Snap[], to: Snap[]) {
    const s = from.pop();
    if (!s) return;
    to.push(copy());
    tiles = s.tiles;
    colors = s.colors;
    tilesDirty = true;
    paletteDirty = true;
  }

  function key(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
    if (!(e.ctrlKey || e.metaKey)) {
      if (e.key === "i") tool = "picker";
      else if (readOnly) return;
      else if (e.key === "b") tool = "pencil";
      else if (e.key === "g") tool = "fill";
      return;
    }
    if (e.key === "z") step(undo, redo);
    else if (e.key === "y") step(redo, undo);
    else if (e.key === "s" && !readOnly) {
      if (tilesDirty || paletteDirty) save();
    }
    else return;
    e.preventDefault();
  }

  function setColor(value: string) {
    // One undo step for a whole colour picker session.
    if (!colorEdit) snapshot();
    colorEdit = true;
    // The DS has 5 bits per channel: snap to what the game can show.
    const c = parseInt(value.slice(1), 16);
    const q = (v: number) => Math.floor((Math.round((v * 31) / 255) * 255) / 31);
    colors[base + color] = (q(c >> 16) << 16) | (q((c >> 8) & 0xff) << 8) | q(c & 0xff);
    paletteDirty = true;
  }

  async function save() {
    busy = true;
    error = null;
    try {
      if (tilesDirty) await saveSheet(chr, tiles);
      if (paletteDirty) await savePalette(pal, colors);
      tilesDirty = paletteDirty = false;
      play("save");
      onSaved();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  async function undoAll() {
    if (!confirmRevert) {
      confirmRevert = true;
      play("select");
      return;
    }
    confirmRevert = false;
    busy = true;
    try {
      await revert(chr);
      play("cancel");
      onSaved();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="pixel-editor">
  <div class="toolbar">
    <div class="group" role="radiogroup" aria-label="Tool">
      {#each [["pencil", "Pencil (B)"], ["fill", "Fill (G)"], ["picker", "Pick (I)"]] as [t, label]}
        <button class:primary={tool === t} aria-pressed={tool === t} disabled={readOnly && t !== "picker"} onclick={() => ((tool = t as Tool), play("select"))}>{label}</button>
      {/each}
    </div>
    {#if bankFor !== null && !rigFrame}
      <div class="group" role="radiogroup" aria-label="View">
        <button class:primary={view === "cells"} aria-pressed={view === "cells"} onclick={() => setView("cells")}>Sprite</button>
        <button class:primary={view === "grid"} aria-pressed={view === "grid"} onclick={() => setView("grid")}>Tiles</button>
      </div>
    {/if}
    <label>
      <span class="muted">Zoom</span>
      <input type="range" min="1" max={maxZoom} bind:value={zoom} />
      <span class="zoomval">{zoom}x</span>
    </label>
    <button title="Fit to the window and centre" onclick={resetView}>Fit</button>
    {#if data && view === "grid" && data.tilesWide === null}
      <label>
        <span class="muted">Width</span>
        <input class="num" type="number" min="1" max="64" bind:value={cols} onchange={() => load(true)} />
      </label>
    {/if}
    {#if palettes.length > 1}
      <label>
        <span class="muted">Palette</span>
        <select value={pal[1]} onchange={(e) => {
            const el = e.currentTarget as HTMLSelectElement;
            onPalette(Number(el.value));
            // If the change was refused the box shows the palette in use.
            el.value = String(pal[1]);
          }}>
          {#each palettes as [name, p]}<option value={p}>{name}</option>{/each}
        </select>
      </label>
    {/if}
    <div class="spacer"></div>
    {#if readOnly}
      <span class="tag">read only</span>
    {:else}
      {#if changed}
        <button disabled={busy} class:danger={confirmRevert} onclick={undoAll} onblur={() => (confirmRevert = false)}>
          {confirmRevert ? "Click again to revert" : "Revert to game"}
        </button>
      {/if}
      <button class="primary" disabled={busy || !(tilesDirty || paletteDirty)} onclick={save}>Save</button>
    {/if}
  </div>

  {#if view === "cells" && bankFor !== null && animFor !== null && !rigFrame}
    <!-- A new player for every picture: nothing carries over from the last. -->
    {#key `${chr.join(":")}/${pal.join(":")}/${bankFor}/${animFor}`}
      <AnimationPlayer {chr} {pal} cer={[chr[0], bankFor]} anr={[chr[0], animFor]} offset={paletteOffset} {version} />
    {/key}
  {/if}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="stage"
    bind:this={stage}
    onpointerdowncapture={pan.down}
    onpointermovecapture={pan.move}
    onpointerupcapture={pan.up}
    onpointercancel={pan.cancel}
    onlostpointercapture={pan.cancel}
    onauxclick={(e) => e.preventDefault()}
  >
    {#if error}
      <p class="bad">{error}</p>
    {/if}
    {#if !data}
      {#if !error}<p class="muted">Loading...</p>{/if}
    {:else}
      <canvas
        class="px sheet"
        bind:this={canvas}
        width={data.width}
        height={data.height}
        style="width: {data.width * zoom}px; height: {data.height * zoom}px; {pan.style}"
        onpointerdown={down}
        onpointermove={move}
        onpointerup={stop}
        onpointercancel={stop}
        oncontextmenu={(e) => e.preventDefault()}
      ></canvas>
    {/if}
  </div>

  {#if data}
    <div class="palette window">
      {#if banks > 1}
        <label>
          <span class="muted">Bank</span>
          <select bind:value={bank}>
            {#each Array(banks) as _, b}<option value={b}>{b}</option>{/each}
          </select>
        </label>
      {/if}
      <div class="swatches">
        {#each swatches as c, i}
          <button
            class="swatch"
            class:on={i === color}
            class:clear={i === 0}
            style="background: {i === 0 ? 'transparent' : hex(c)}"
            title={i === 0 ? "0: transparent" : `${i}: ${hex(c)}`}
            onclick={() => ((color = i), (colorEdit = false), play("select"))}
            aria-label="Colour {i}"
          ></button>
        {/each}
      </div>
      {#if !readOnly && color !== 0}
        <label class="edit">
          <span class="muted">Colour {color}</span>
          <input type="color" value={hex(colors[base + color] ?? 0)} oninput={(e) => setColor((e.currentTarget as HTMLInputElement).value)} onchange={() => (colorEdit = false)} />
        </label>
      {/if}
      <span class="muted hint">{readOnly ? "Click: pick colour." : "Right click: pick colour. Ctrl+Z: undo. Ctrl+S: save. Middle drag: move."}</span>
    </div>
  {/if}
</div>

<style>
  /* The stage takes the room left over and scrolls; the bars around it
     stay put whatever the zoom. */
  .pixel-editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6rem;
    padding: 0.35rem 0.9rem;
    border-bottom: 2px solid var(--outline);
  }
  .toolbar label,
  .palette label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .group {
    display: flex;
    gap: 0.25rem;
  }
  .toolbar button {
    padding: 0.2rem 0.7rem;
  }
  .num {
    width: 4.5rem;
  }
  .toolbar button.danger {
    border-color: var(--bad, #c33);
  }
  .zoomval {
    min-width: 2.2rem;
    font-variant-numeric: tabular-nums;
  }
  .spacer {
    flex: 1;
  }
  .stage {
    flex: 1 1 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 1rem;
    min-width: 0;
    background-color: var(--stage);
    background-image: linear-gradient(45deg, var(--stage-check) 25%, transparent 25%),
      linear-gradient(-45deg, var(--stage-check) 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, var(--stage-check) 75%),
      linear-gradient(-45deg, transparent 75%, var(--stage-check) 75%);
    background-size: 16px 16px;
    background-position: 0 0, 0 8px, 8px -8px, -8px 0;
  }
  .sheet {
    display: block;
    cursor: crosshair;
  }
  .pixel-editor > :global(:not(.stage)) {
    flex: none;
  }
  .palette {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.75rem;
    margin: 0.6rem 0.9rem;
    padding: 0.6rem 0.75rem;
  }
  .swatches {
    display: grid;
    grid-template-columns: repeat(16, 1.6rem);
    gap: 3px;
    /* A 256-colour palette scrolls instead of eating the window. */
    max-height: 7.6rem;
    overflow-y: auto;
    padding: 5px;
  }
  .swatch {
    width: 1.6rem;
    height: 1.6rem;
    padding: 0;
    border: 2px solid var(--outline);
    border-radius: 4px;
    box-shadow: none;
  }
  .swatch.clear {
    background-image: linear-gradient(45deg, #666 25%, transparent 25%, transparent 75%, #666 75%),
      linear-gradient(45deg, #666 25%, transparent 25%, transparent 75%, #666 75%) !important;
    background-size: 8px 8px !important;
    background-position: 0 0, 4px 4px !important;
  }
  .swatch.on {
    outline: 3px solid var(--gold);
    outline-offset: 1px;
  }
  .hint {
    font-size: 0.85rem;
  }
</style>
