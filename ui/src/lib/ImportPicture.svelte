<script lang="ts" module>
  import type { Layout, Ref } from "./backend";

  /// What a PNG is laid over: the picture as the editor lays it out.
  export interface ImportTarget {
    chr: Ref;
    pal: Ref;
    layout: Layout;
    /// Added to every pixel's palette bank.
    bank: number;
    /// What the dialog calls it.
    title: string;
    /// The editor holds edits that are not saved; the import drops them.
    unsaved?: boolean;
  }
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { importPicture, readPng, sheetData, type ImportMode, type ImportSummary } from "./backend";
  import { play } from "./sfx";

  let {
    target,
    path,
    onDone,
    onClose,
  }: { target: ImportTarget; path: string; onDone: () => void; onClose: () => void } = $props();

  let dialog = $state<HTMLDialogElement | null>(null);
  let canvas = $state<HTMLCanvasElement | null>(null);
  let src = $state<string | null>(null);
  let size = $state<[number, number] | null>(null);
  let have = $state<[number, number] | null>(null);
  let mode = $state<ImportMode>("keep");
  let error = $state<string | null>(null);
  let busy = $state(false);
  let done = $state<ImportSummary | null>(null);

  const fileName = $derived(path.split(/[\\/]/).pop() ?? path);
  // Same size, or a whole multiple that is scaled down.
  const fits = $derived.by(() => {
    if (!size || !have) return null;
    if (size[0] === have[0] && size[1] === have[1]) return "same";
    const k = size[0] / have[0];
    return Number.isInteger(k) && k > 1 && size[1] === have[1] * k ? "scaled" : "wrong";
  });

  async function drawCurrent() {
    const data = await sheetData(target.chr, target.pal, target.layout);
    have = [data.width, data.height];
    if (!canvas) return;
    canvas.width = data.width;
    canvas.height = data.height;
    const ctx = canvas.getContext("2d")!;
    const img = ctx.createImageData(data.width, data.height);
    data.map.forEach((m, i) => {
      if (m < 0) return;
      const v = data.tiles[m >> 6][m & 63];
      if (v === 0) return;
      const bank = data.bpp === 4 ? Math.min(data.banks[i] + target.bank, 255) * 16 : 0;
      const c = data.colors[bank + v] ?? 0xff00ff;
      img.data.set([c >> 16, (c >> 8) & 255, c & 255, 255], i * 4);
    });
    ctx.putImageData(img, 0, 0);
  }

  onMount(() => {
    dialog?.showModal();
    drawCurrent().catch((e) => (error = String(e)));
    readPng(path)
      .then((bytes) => (src = URL.createObjectURL(new Blob([bytes], { type: "image/png" }))))
      .catch((e) => (error = String(e)));
    return () => {
      if (src) URL.revokeObjectURL(src);
    };
  });

  async function run() {
    busy = true;
    error = null;
    try {
      done = await importPicture(target.chr, target.pal, target.layout, path, mode, target.bank);
      play("save");
      onDone();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialog}
  class="import window"
  onclose={onClose}
  onclick={(e) => {
    if (e.target === dialog) dialog?.close();
  }}
  aria-label="Import PNG"
>
  <header>
    <h2>Import PNG</h2>
    <span class="muted">{target.title}</span>
  </header>

  <div class="pair">
    <figure>
      <div class="frame">
        {#if src}
          <img {src} alt="The PNG" onload={(e) => {
              const img = e.currentTarget as HTMLImageElement;
              size = [img.naturalWidth, img.naturalHeight];
            }} />
        {/if}
      </div>
      <figcaption>
        <span class="name" title={path}>{fileName}</span>
        {#if size}<span class="muted">{size[0]}x{size[1]}</span>{/if}
      </figcaption>
    </figure>
    <figure>
      <div class="frame"><canvas bind:this={canvas}></canvas></div>
      <figcaption>
        <span>Now</span>
        {#if have}<span class="muted">{have[0]}x{have[1]}</span>{/if}
      </figcaption>
    </figure>
  </div>

  {#if fits === "wrong"}
    <p class="bad">The PNG is {size![0]}x{size![1]} and the picture is {have![0]}x{have![1]}. The sizes must match.</p>
  {:else if fits === "scaled"}
    <p class="muted">The PNG is larger by a whole factor and is scaled down.</p>
  {/if}

  {#if !done && target.unsaved}
    <p class="muted">The edits in the editor that are not saved are dropped.</p>
  {/if}
  {#if !done}
    <div class="modes" role="radiogroup" aria-label="Palette">
      <label>
        <input type="radio" bind:group={mode} value="keep" />
        <span>Keep colours</span>
        <small class="muted">Each pixel takes the nearest colour of the palette. Transparent pixels stay transparent.</small>
      </label>
      <label>
        <input type="radio" bind:group={mode} value="replace" />
        <span>Use the PNG's colours</span>
        <small class="muted">The palette is replaced by the PNG's colours, reduced to fit.</small>
      </label>
    </div>
  {:else}
    <p class="result">
      {plural(done.pixels, "pixel")} written, {plural(done.colorsUsed, "colour")} used{done.colorsMerged
        ? `, ${plural(done.colorsMerged, "colour")} merged`
        : ""}{done.paletteChanged ? ", palette changed" : ""}.
    </p>
  {/if}
  {#if error}<p class="bad">{error}</p>{/if}

  <footer>
    {#if done}
      <button class="primary" onclick={() => dialog?.close()}>Close</button>
    {:else}
      <button onclick={() => dialog?.close()}>Cancel</button>
      <button class="primary" disabled={busy || fits === "wrong" || !size || !have} onclick={run}>Import</button>
    {/if}
  </footer>
</dialog>

<style>
  .import {
    width: min(38rem, calc(100vw - 2rem));
    max-height: calc(100vh - 4rem);
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    color: inherit;
    overflow-y: auto;
  }
  .import:not([open]) {
    display: none;
  }
  .import::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
  }
  h2 {
    margin: 0;
  }
  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
  }
  figure {
    margin: 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .frame {
    height: 14rem;
    display: grid;
    place-items: center;
    overflow: hidden;
    background-color: var(--stage);
    border: 2px solid var(--outline);
    border-radius: 6px;
  }
  .frame img,
  .frame canvas {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    image-rendering: pixelated;
  }
  figcaption {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .modes {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .modes label {
    display: grid;
    grid-template-columns: auto 1fr;
    column-gap: 0.5rem;
    align-items: baseline;
  }
  .modes small {
    grid-column: 2;
  }
  p {
    margin: 0;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
