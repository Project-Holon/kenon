<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import RigPreview from "./RigPreview.svelte";
  import { asset } from "./backend";
  import { play } from "./sfx";
  import { copyText } from "./exporter";
  import { openMenu } from "morphe/menu.svelte";
  import { tileLabel, tileRgb } from "./terrain";

  // Everything the mod changes, the game's version next to yours, and a
  // way to undo each change or all of them.
  let {
    onClose,
    onChanged,
    onRefresh,
  }: { onClose: () => void; onChanged: () => void; onRefresh: () => void } = $props();

  interface DiffLine {
    label: string;
    old: string;
    new: string;
  }
  interface TileDiff {
    width: number;
    height: number;
    old: [number, number][];
    new: [number, number][];
  }
  interface ChangeInfo {
    path: string;
    member: [number, number] | null;
    category: string;
    title: string;
    lines: DiffLine[];
    more: number;
    preview: string | null;
    tiles: TileDiff | null;
  }

  let list = $state<ChangeInfo[] | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  // Open or shut by the user; untouched entries follow `OPEN_FIRST`.
  let toggled = $state<Record<string, boolean>>({});
  // Pictures load when an entry opens: only the first few start open, and
  // the list grows by pages.
  const OPEN_FIRST = 5;
  const PAGE = 100;
  let limit = $state(PAGE);
  let confirmAll = $state(false);
  let dialog = $state<HTMLDialogElement | null>(null);
  // Defeats the webview's image cache: a picture can change between opens.
  let stamp = $state(Date.now());

  async function load() {
    try {
      list = await invoke<ChangeInfo[]>("describe_changes");
      stamp = Date.now();
      // Listing drops changes that change nothing: the count may move.
      onRefresh();
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    dialog?.showModal();
    load();
  });

  async function undo(paths: string[]) {
    busy = true;
    error = null;
    try {
      for (const path of paths) await invoke("revert_change", { path });
      play("cancel");
      onChanged();
      confirmAll = false;
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  const menu = (e: MouseEvent, c: ChangeInfo) =>
    openMenu(e, [
      { label: "Undo this change", disabled: busy, run: () => undo([c.path]) },
      { label: "Copy title", run: () => copyText(c.title) },
    ]);

  const groups = $derived.by(() => {
    const out = new Map<string, ChangeInfo[]>();
    for (const c of (list ?? []).slice(0, limit)) out.set(c.category, [...(out.get(c.category) ?? []), c]);
    return [...out];
  });
  const position = $derived(new Map((list ?? []).map((c, i) => [c.path, i])));

  const withStamp = (path: string) => `${path}${path.includes("?") ? "&" : "?"}v=${stamp}`;

  // Tile changes summed up by kind: "Grass -> Water: 12 tiles".
  function tileSummary(t: TileDiff): [string, string, number][] {
    const counts = new Map<string, [string, string, number]>();
    t.old.forEach(([ot, of], i) => {
      const [nt, nf] = t.new[i];
      if (ot === nt && of === nf) return;
      const key = `${ot}:${of}>${nt}:${nf}`;
      const c = counts.get(key) ?? [tileLabel(ot, of), tileLabel(nt, nf), 0];
      c[2]++;
      counts.set(key, c);
    });
    return [...counts.values()].sort((a, b) => b[2] - a[2]);
  }

  // Draws a tile grid; changed tiles get a gold frame on the new side.
  function tiles(canvas: HTMLCanvasElement, args: { t: TileDiff; side: "old" | "new" }) {
    const draw = ({ t, side }: typeof args) => {
      const px = 6;
      canvas.width = t.width * px;
      canvas.height = t.height * px;
      const ctx = canvas.getContext("2d")!;
      const cells = side === "old" ? t.old : t.new;
      cells.forEach(([tt, f], i) => {
        const [r, g, b] = tileRgb(tt, f);
        ctx.fillStyle = `rgb(${r} ${g} ${b})`;
        ctx.fillRect((i % t.width) * px, Math.floor(i / t.width) * px, px, px);
      });
      ctx.strokeStyle = side === "new" ? "#f5c400" : "#e5484d";
      ctx.lineWidth = 1.5;
      t.old.forEach(([ot, of], i) => {
        const [nt, nf] = t.new[i];
        if (ot !== nt || of !== nf) ctx.strokeRect((i % t.width) * px + 0.75, Math.floor(i / t.width) * px + 0.75, px - 1.5, px - 1.5);
      });
    };
    draw(args);
    return { update: draw };
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialog}
  class="changes window"
  onclose={onClose}
  onclick={(e) => {
    if (e.target === dialog) dialog?.close();
  }}
  aria-label="Your changes"
>
  <header>
    <h2>Your changes</h2>
    <span class="muted">{list ? `${list.length} change${list.length === 1 ? "" : "s"}` : ""}</span>
    <div class="spacer"></div>
    {#if list?.length}
      {#if confirmAll}
        <span>Put back the game's version everywhere?</span>
        <button class="danger" disabled={busy} onclick={() => undo(list!.map((c) => c.path))}>Undo all</button>
        <button onclick={() => (confirmAll = false)}>Keep</button>
      {:else}
        <button onclick={() => (confirmAll = true)}>Undo all</button>
      {/if}
    {/if}
    <button onclick={() => dialog?.close()}>Close</button>
  </header>

  {#if error}<p class="bad pad">{error}</p>{/if}

  <div class="body">
    {#if list === null}
      <p class="muted">Loading...</p>
    {:else if !list.length}
      <p class="muted">No changes yet. What you save in an editor is listed here.</p>
    {:else}
      {#each groups as [category, items]}
        <section>
          <h3>{category} <span class="muted">{items.length}</span></h3>
          {#each items as c (c.path)}
            {@const open = toggled[c.path] ?? (position.get(c.path) ?? 0) < OPEN_FIRST}
            <article class="change" oncontextmenu={(e) => menu(e, c)}>
              <div class="row">
                <button class="title quiet" onclick={() => (toggled[c.path] = !open)} aria-expanded={open} title={c.path}>
                  <span class="caret" class:open></span>
                  <strong>{c.title}</strong>
                </button>
                <button disabled={busy} onclick={() => undo([c.path])} title="Put back the game's version">Undo</button>
              </div>
              {#if open}
                {#if c.preview}
                  <div class="compare">
                    <figure>
                      {#if c.preview.startsWith("rigframe/")}<span class="anim"><RigPreview path={`game/${c.preview}`} alt="Game" /></span>{:else}<img class="px" src={asset(withStamp(`game/${c.preview}`))} alt="Game" />{/if}
                      <figcaption>Game</figcaption>
                    </figure>
                    <span class="arrow">-></span>
                    <figure>
                      {#if c.preview.startsWith("rigframe/")}<span class="anim"><RigPreview path={c.preview} {stamp} alt="Yours" /></span>{:else}<img class="px" src={asset(withStamp(c.preview))} alt="Yours" />{/if}
                      <figcaption>Yours</figcaption>
                    </figure>
                  </div>
                {/if}
                {#if c.tiles}
                  <div class="compare">
                    <figure>
                      <canvas use:tiles={{ t: c.tiles, side: "old" }}></canvas>
                      <figcaption>Game</figcaption>
                    </figure>
                    <span class="arrow">-></span>
                    <figure>
                      <canvas use:tiles={{ t: c.tiles, side: "new" }}></canvas>
                      <figcaption>Yours</figcaption>
                    </figure>
                  </div>
                  <table>
                    <tbody>
                      {#each tileSummary(c.tiles) as [from, to, n]}
                        <tr>
                          <th>{n} tile{n === 1 ? "" : "s"}</th>
                          <td class="old">{from}</td>
                          <td class="arrow">-></td>
                          <td class="new">{to}</td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                {:else if c.lines.length}
                  <table>
                    <tbody>
                      {#each c.lines as l}
                        <tr>
                          <th>{l.label}</th>
                          <td class="old">{l.old}</td>
                          <td class="arrow">{l.old ? "->" : ""}</td>
                          <td class="new">{l.new}</td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                  {#if c.more}<p class="muted small">{c.more} more</p>{/if}
                {/if}
              {/if}
            </article>
          {/each}
        </section>
      {/each}
      {#if list.length > limit}
        <button class="more" onclick={() => (limit += PAGE)}>Show more ({list.length - limit} left)</button>
      {/if}
    {/if}
  </div>
</dialog>

<style>
  .changes {
    width: min(60rem, calc(100vw - 2rem));
    max-height: calc(100vh - 4rem);
    padding: 0;
    display: flex;
    flex-direction: column;
    color: inherit;
  }
  .changes:not([open]) {
    display: none;
  }
  .changes::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.6rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  header h2 {
    margin: 0;
    font-size: 1.1rem;
  }
  header button {
    padding: 0.2rem 0.7rem;
  }
  .spacer {
    flex: 1;
  }
  .pad {
    padding: 0.5rem 1rem 0;
    margin: 0;
  }
  .more {
    display: block;
    margin: 0.5rem auto 0;
  }
  .body {
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0.75rem 1rem 1rem;
    min-height: 0;
  }
  section h3 {
    margin: 0.8rem 0 0.4rem;
    font-size: 0.95rem;
  }
  .change {
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    margin-bottom: 0.5rem;
    overflow: hidden;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3rem 0.5rem;
  }
  .row button:last-child {
    padding: 0.15rem 0.7rem;
  }
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    text-align: left;
    box-shadow: none;
  }
  .title strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .caret {
    width: 0;
    height: 0;
    border-left: 5px solid currentColor;
    border-top: 4px solid transparent;
    border-bottom: 4px solid transparent;
    opacity: 0.6;
    transition: transform 0.12s;
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .compare {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    padding: 0.6rem;
    border-top: 1px solid color-mix(in srgb, var(--outline) 40%, transparent);
    background: var(--stage);
  }
  figure {
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
  }
  figure img,
  figure canvas {
    max-width: 24rem;
    max-height: 16rem;
    min-width: 96px;
    image-rendering: pixelated;
    object-fit: contain;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    background: var(--bg);
  }
  .anim {
    display: block;
    padding: 4px;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    background: var(--bg);
    zoom: 2;
  }
  figcaption {
    font-size: 0.8rem;
    color: var(--muted);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
    font-size: 0.85rem;
  }
  tr {
    border-top: 1px solid color-mix(in srgb, var(--outline) 40%, transparent);
  }
  th {
    width: 30%;
    text-align: left;
    font-weight: 600;
    padding: 0.25rem 0.6rem;
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  td {
    padding: 0.25rem 0.4rem;
    overflow-wrap: anywhere;
  }
  .old {
    background: color-mix(in srgb, #e5484d 16%, transparent);
    text-decoration: line-through;
    text-decoration-color: color-mix(in srgb, #e5484d 60%, transparent);
  }
  .new {
    background: color-mix(in srgb, #30a46c 18%, transparent);
  }
  .arrow {
    width: 2rem;
    text-align: center;
    opacity: 0.6;
  }
  .small {
    font-size: 0.8rem;
    padding: 0 0.6rem 0.4rem;
  }
  .danger {
    background: #e5484d;
    color: #fff;
  }
</style>
