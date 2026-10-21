<script lang="ts">
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
  let maxZoom = $state(12);
  // svelte-ignore state_referenced_locally
  let bank = $state(paletteOffset);
  let color = $state(1);
  let tool = $state<Tool>("pencil");
  let tilesDirty = $state(false);
  let paletteDirty = $state(false);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let canvas = $state<HTMLCanvasElement | null>(null);
  let stage = $state<HTMLDivElement | null>(null);

  let undo: { tiles: number[][]; colors: number[] }[] = [];
  let redo: { tiles: number[][]; colors: number[] }[] = [];
  let drawing = false;
  let fitted = false;

  const banks = $derived(data?.bpp === 4 ? Math.max(1, Math.floor(colors.length / 16)) : 1);
  const base = $derived(data?.bpp === 4 ? bank * 16 : 0);
  const swatches = $derived(data?.bpp === 4 ? colors.slice(base, base + 16) : colors);

  async function load(keepEdits: boolean) {
    error = null;
    try {
      const layout: Layout =
        view === "cells" && bankFor !== null
          ? { kind: "cells", cer: [chr[0], bankFor] }
          : { kind: "grid", cols };
      const next = await sheetData(chr, pal, layout);
      if (!keepEdits || !data) {
        tiles = next.tiles.map((t) => t.slice());
        colors = next.colors.slice();
      }
      data = next;
      fitted = false;
      fit();
    } catch (e) {
      error = String(e);
    }
  }

  onMount(async () => {
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
    load(true);
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
    return (view === "cells" ? data.banks[i] + paletteOffset : bank) * 16;
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

  function snapshot() {
    undo.push({ tiles: tiles.map((t) => t.slice()), colors: colors.slice() });
    if (undo.length > 100) undo.shift();
    redo = [];
  }

  function paint(i: number) {
    const src = data!.map[i];
    if (tiles[src >> 6][src & 63] === color) return;
    tiles[src >> 6][src & 63] = color;
    tilesDirty = true;
  }

  function fill(start: number) {
    const { width, height, map } = data!;
    const target = tiles[map[start] >> 6][map[start] & 63];
    if (target === color) return;
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
    tilesDirty = true;
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
    if (readOnly) return;
    snapshot();
    if (tool === "fill") {
      fill(i);
      return;
    }
    drawing = true;
    (e.currentTarget as HTMLCanvasElement).setPointerCapture(e.pointerId);
    paint(i);
  }

  function move(e: PointerEvent) {
    if (!drawing) return;
    const i = at(e);
    if (i !== null) paint(i);
  }

  function step(from: typeof undo, to: typeof undo) {
    const s = from.pop();
    if (!s) return;
    to.push({ tiles: tiles.map((t) => t.slice()), colors: colors.slice() });
    tiles = s.tiles;
    colors = s.colors;
    tilesDirty = true;
    paletteDirty = true;
  }

  function key(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
    if (!(e.ctrlKey || e.metaKey)) {
      if (e.key === "b") tool = "pencil";
      else if (e.key === "g") tool = "fill";
      else if (e.key === "i") tool = "picker";
      return;
    }
    if (e.key === "z") step(undo, redo);
    else if (e.key === "y") step(redo, undo);
    else if (e.key === "s" && !readOnly) save();
    else return;
    e.preventDefault();
  }

  function setColor(value: string) {
    snapshot();
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
        <button class:primary={tool === t} disabled={readOnly && t !== "picker"} onclick={() => ((tool = t as Tool), play("select"))}>{label}</button>
      {/each}
    </div>
    {#if bankFor !== null}
      <div class="group" role="radiogroup" aria-label="View">
        <button class:primary={view === "cells"} onclick={() => setView("cells")}>Sprite</button>
        <button class:primary={view === "grid"} onclick={() => setView("grid")}>Tiles</button>
      </div>
    {/if}
    <label>
      <span class="muted">Zoom</span>
      <input type="range" min="1" max={maxZoom} bind:value={zoom} />
    </label>
    {#if data && view === "grid" && data.tilesWide === null}
      <label>
        <span class="muted">Width</span>
        <input class="num" type="number" min="1" max="64" bind:value={cols} onchange={() => load(true)} />
      </label>
    {/if}
    <label>
      <span class="muted">Palette</span>
      <select value={pal[1]} onchange={(e) => onPalette(Number((e.currentTarget as HTMLSelectElement).value))}>
        {#each palettes as [name, p]}<option value={p}>{name}</option>{/each}
      </select>
    </label>
    <div class="spacer"></div>
    {#if readOnly}
      <span class="tag">read only</span>
    {:else}
      {#if changed}<button disabled={busy} onclick={undoAll}>Revert to game</button>{/if}
      <button class="primary" disabled={busy || !(tilesDirty || paletteDirty)} onclick={save}>Save</button>
    {/if}
  </div>

  {#if view === "cells" && bankFor !== null && animFor !== null}
    <!-- A new player for every picture: nothing carries over from the last. -->
    {#key `${chr.join(":")}/${pal.join(":")}/${bankFor}/${animFor}`}
      <AnimationPlayer {chr} {pal} cer={[chr[0], bankFor]} anr={[chr[0], animFor]} offset={paletteOffset} version={tilesDirty || paletteDirty ? -1 : 0} />
    {/key}
  {/if}
  <div class="stage" bind:this={stage}>
    {#if error}
      <p class="bad">{error}</p>
    {:else if !data}
      <p class="muted">Loading...</p>
    {:else}
      <canvas
        class="px sheet"
        bind:this={canvas}
        width={data.width}
        height={data.height}
        style="width: {data.width * zoom}px; height: {data.height * zoom}px"
        onpointerdown={down}
        onpointermove={move}
        onpointerup={() => (drawing = false)}
        oncontextmenu={(e) => e.preventDefault()}
      ></canvas>
      {#if view === "grid" && bankFor !== null}
        <p class="muted note">
          Tiles view: this sheet is stored piece by piece, so it only makes a picture in the Sprite view.
        </p>
      {/if}
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
            onclick={() => ((color = i), play("select"))}
            aria-label="Colour {i}"
          ></button>
        {/each}
      </div>
      {#if !readOnly && color !== 0}
        <label class="edit">
          <span class="muted">Colour {color}</span>
          <input type="color" value={hex(colors[base + color] ?? 0)} onchange={(e) => setColor((e.currentTarget as HTMLInputElement).value)} />
        </label>
      {/if}
      <span class="muted hint">Right click picks a colour. Ctrl+Z undoes, Ctrl+S saves.</span>
    </div>
  {/if}
</div>

<style>
  .pixel-editor {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    height: 100%;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 0.9rem;
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
  .num {
    width: 4.5rem;
  }
  .spacer {
    flex: 1;
  }
  .stage {
    overflow-x: hidden;
    overflow-y: auto;
    padding: 1rem;
    min-width: 0;
    background-color: #0c0d10;
    background-image: linear-gradient(45deg, #15171c 25%, transparent 25%),
      linear-gradient(-45deg, #15171c 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #15171c 75%),
      linear-gradient(-45deg, transparent 75%, #15171c 75%);
    background-size: 16px 16px;
    background-position: 0 0, 0 8px, 8px -8px, -8px 0;
  }
  .sheet {
    display: block;
    cursor: crosshair;
  }
  .note {
    margin-top: 0.75rem;
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
