<script lang="ts">
  import { onMount } from "svelte";
  import { revert, savePalette, saveSheet, sheetData, type Ref, type SheetData } from "../lib/backend";
  import { play } from "../lib/sfx";

  let {
    chr,
    pal,
    readOnly,
    changed,
    palettes,
    onPalette,
    onSaved,
  }: {
    chr: Ref;
    pal: Ref;
    readOnly: boolean;
    changed: boolean;
    palettes: number[];
    onPalette: (index: number) => void;
    onSaved: () => void;
  } = $props();

  type Tool = "pencil" | "fill" | "picker";

  let data = $state<SheetData | null>(null);
  let tiles = $state<number[][]>([]);
  let colors = $state<number[]>([]);
  let cols = $state(16);
  let zoom = $state(4);
  // The largest zoom that fits the pane: the sheet never scrolls sideways.
  let maxZoom = $state(12);
  let bank = $state(0);
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

  const rows = $derived(Math.max(1, Math.ceil(tiles.length / cols)));
  const width = $derived(cols * 8);
  const height = $derived(rows * 8);
  const banks = $derived(data?.bpp === 4 ? Math.max(1, Math.floor(colors.length / 16)) : 1);
  const base = $derived(data?.bpp === 4 ? bank * 16 : 0);
  const swatches = $derived(data?.bpp === 4 ? colors.slice(base, base + 16) : colors);

  onMount(async () => {
    try {
      data = await sheetData(chr, pal);
      tiles = data.tiles.map((t) => t.slice());
      colors = data.colors.slice();
      cols = data.tilesWide ?? Math.min(16, Math.max(1, tiles.length));
      fit();
    } catch (e) {
      error = String(e);
    }
  });

  function fit() {
    const avail = (stage?.clientWidth ?? 600) - 32;
    maxZoom = Math.max(1, Math.min(16, Math.floor(avail / (cols * 8))));
    zoom = fitted ? Math.min(zoom, maxZoom) : maxZoom;
    fitted = true;
  }

  // Refit when the pane or the layout width changes.
  $effect(() => {
    void cols;
    if (!stage) return;
    const observer = new ResizeObserver(() => fit());
    observer.observe(stage);
    return () => observer.disconnect();
  });

  const hex = (c: number) => `#${c.toString(16).padStart(6, "0")}`;

  // Redraw whenever the pixels, palette, layout or bank change.
  $effect(() => {
    if (!canvas || !data) return;
    const ctx = canvas.getContext("2d")!;
    const img = ctx.createImageData(width, height);
    tiles.forEach((tile, t) => {
      const tx = (t % cols) * 8;
      const ty = Math.floor(t / cols) * 8;
      for (let p = 0; p < 64; p++) {
        const v = tile[p];
        if (v === 0) continue;
        const c = colors[base + v] ?? 0xff00ff;
        const i = ((ty + (p >> 3)) * width + tx + (p & 7)) * 4;
        img.data[i] = c >> 16;
        img.data[i + 1] = (c >> 8) & 0xff;
        img.data[i + 2] = c & 0xff;
        img.data[i + 3] = 255;
      }
    });
    ctx.putImageData(img, 0, 0);
  });

  function at(e: PointerEvent): { t: number; p: number } | null {
    const r = (e.currentTarget as HTMLCanvasElement).getBoundingClientRect();
    const x = Math.floor(((e.clientX - r.left) / r.width) * width);
    const y = Math.floor(((e.clientY - r.top) / r.height) * height);
    if (x < 0 || y < 0 || x >= width || y >= height) return null;
    const t = Math.floor(y / 8) * cols + Math.floor(x / 8);
    if (t >= tiles.length) return null;
    return { t, p: (y % 8) * 8 + (x % 8) };
  }

  function snapshot() {
    undo.push({ tiles: tiles.map((t) => t.slice()), colors: colors.slice() });
    if (undo.length > 100) undo.shift();
    redo = [];
  }

  function paint(spot: { t: number; p: number }) {
    if (tiles[spot.t][spot.p] === color) return;
    tiles[spot.t][spot.p] = color;
    tilesDirty = true;
  }

  function fill(spot: { t: number; p: number }) {
    const target = tiles[spot.t][spot.p];
    if (target === color) return;
    const px = (t: number, p: number) => ({ x: (t % cols) * 8 + (p & 7), y: Math.floor(t / cols) * 8 + (p >> 3) });
    const start = px(spot.t, spot.p);
    const stack = [start];
    const seen = new Set<number>();
    while (stack.length) {
      const { x, y } = stack.pop()!;
      if (x < 0 || y < 0 || x >= width || y >= height || seen.has(y * width + x)) continue;
      seen.add(y * width + x);
      const t = Math.floor(y / 8) * cols + Math.floor(x / 8);
      const p = (y % 8) * 8 + (x % 8);
      if (t >= tiles.length || tiles[t][p] !== target) continue;
      tiles[t][p] = color;
      stack.push({ x: x + 1, y }, { x: x - 1, y }, { x, y: y + 1 }, { x, y: y - 1 });
    }
    tilesDirty = true;
  }

  function down(e: PointerEvent) {
    const spot = at(e);
    if (!spot) return;
    if (e.button === 2 || tool === "picker") {
      color = tiles[spot.t][spot.p];
      play("select");
      return;
    }
    if (readOnly) return;
    snapshot();
    if (tool === "fill") {
      fill(spot);
      return;
    }
    drawing = true;
    (e.currentTarget as HTMLCanvasElement).setPointerCapture(e.pointerId);
    paint(spot);
  }

  function move(e: PointerEvent) {
    if (!drawing) return;
    const spot = at(e);
    if (spot) paint(spot);
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
    <label>
      <span class="muted">Zoom</span>
      <input type="range" min="1" max={maxZoom} bind:value={zoom} />
    </label>
    {#if data && data.tilesWide === null}
      <label>
        <span class="muted">Width</span>
        <input class="num" type="number" min="1" max="64" bind:value={cols} />
      </label>
    {/if}
    <label>
      <span class="muted">Palette</span>
      <select value={pal[1]} onchange={(e) => onPalette(Number((e.currentTarget as HTMLSelectElement).value))}>
        {#each palettes as p}<option value={p}>#{p}</option>{/each}
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

  <div class="stage" bind:this={stage}>
    {#if error}
      <p class="bad">{error}</p>
    {:else if !data}
      <p class="muted">Loading...</p>
    {:else}
      <canvas
        class="px sheet"
        bind:this={canvas}
        {width}
        {height}
        style="width: {width * zoom}px; height: {height * zoom}px"
        onpointerdown={down}
        onpointermove={move}
        onpointerup={() => (drawing = false)}
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
      <div class="swatches" class:wide={data.bpp === 8}>
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
    </div>
  {/if}
</div>

<style>
  .pixel-editor {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
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
    outline: 2px solid var(--outline);
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
</style>
