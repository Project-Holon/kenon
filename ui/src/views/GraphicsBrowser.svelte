<script lang="ts">
  import { onMount } from "svelte";
  import {
    archives,
    asset,
    changedMembers,
    members,
    namedSheets,
    screens,
    type ArchiveInfo,
    type MemberInfo,
    type NamedSheet,
    type Ref,
  } from "../lib/backend";
  import { play } from "../lib/sfx";
  import PixelEditor from "./PixelEditor.svelte";
  import RigPlayer from "./RigPlayer.svelte";
  import VirtualList from "../lib/VirtualList.svelte";
  import Thumb from "../lib/Thumb.svelte";
  import { debounced } from "../lib/cache.svelte";

  let {
    archive,
    readOnly,
    version,
    onSaved,
  }: { archive: number | null; readOnly: boolean; version: number; onSaved: () => void } = $props();

  // One picture of the list: a named sheet when the archive's layout is
  // known (names read from the ROM), else a bare member.
  interface Item {
    key: string;
    label: string;
    chr: number;
    pal: number | null;
    palettes: [string, number][];
    offset: number;
    /// Known layout: a cell bank, null for a plain picture, undefined to find.
    cells?: number | null;
    anims?: number | null;
    rig?: [number, number, number, number] | null;
    /// A background screen drawn with this sheet.
    scr?: number;
  }

  // The archive shown; fixed per category (the view is rebuilt on a new one).
  // svelte-ignore state_referenced_locally
  let arc = $state<number | null>(archive);
  let choices = $state<ArchiveInfo[]>([]);
  let list = $state<MemberInfo[]>([]);
  let items = $state<Item[]>([]);
  let selected = $state<Item | null>(null);
  let palette = $state<number | null>(null);
  let filter = $state("");
  let error = $state<string | null>(null);
  // Battle sprites: edit the still picture, or the frame the animation shows.
  let editMode = $state<"picture" | "animation">("picture");
  let rigFrameIndex = $state(0);
  // Badge the animated pictures when only some of the list has one.
  const animated = (i: Item) => !!i.rig || (i.anims !== undefined && i.anims !== null);
  const someAnimated = $derived(items.some(animated) && !items.every(animated));
  let changedSet = $state<Set<number>>(new Set());

  const palettes = $derived(list.filter((m) => m.kind === "NCLR"));
  const query = debounced(() => filter);
  const shown = $derived.by(() => {
    const q = query.value.trim().toLowerCase();
    return q ? items.filter((i) => i.label.toLowerCase().includes(q)) : items;
  });

  // The palette a bare sheet is drawn with by default: the nearest palette
  // before it in the archive, else the nearest after.
  function pairedPalette(ms: MemberInfo[], index: number): number | null {
    let before: number | null = null;
    let after: number | null = null;
    for (const p of ms) {
      if (p.kind !== "NCLR") continue;
      if (p.index < index) before = p.index;
      else if (after === null) after = p.index;
    }
    return before ?? after;
  }

  async function load(id: number) {
    error = null;
    list = [];
    items = [];
    selected = null;
    try {
      changedMembers(id).then((c) => (changedSet = new Set(c)));
      // A named archive lists at once; only a bare one needs the full
      // member listing (to find its sheets and palettes).
      const named = await namedSheets(id);
      const ms = named ? [] : await members(id);
      const scrs = named ? [] : await screens(id);
      list = ms;
      // Bare archives: the assembled screens first (menus are built from
      // them), then each sheet on its own.
      const screenItems: Item[] = scrs.map((s) => ({
        key: `s${s.scr}`,
        label: `Screen #${s.scr} (${s.width}x${s.height})`,
        chr: s.chr,
        pal: s.pal,
        palettes: [],
        offset: 0,
        scr: s.scr,
      }));
      items = named
        ? named.map((n: NamedSheet) => ({
            key: `${n.chr}:${n.label}`,
            label: n.label,
            chr: n.chr,
            pal: n.pal,
            palettes: [["Normal", n.pal] as [string, number], ...n.palettes],
            offset: n.paletteOffset,
            cells: n.cells,
            anims: n.anims,
            rig: n.rig,
          }))
        : ms
            .filter((m) => m.kind === "NCGR")
            .map((m) => ({
              key: String(m.index),
              label: `#${m.index}`,
              chr: m.index,
              pal: pairedPalette(ms, m.index),
              palettes: [],
              offset: 0,
            }));
      if (!named) items = [...screenItems, ...items];
      if (items[0]) choose(items[0], false);
    } catch (e) {
      error = String(e);
    }
  }

  function choose(item: Item, sound = true) {
    if (sound) play("select");
    selected = item;
    palette = item.pal;
  }

  onMount(async () => {
    if (arc === null) {
      choices = (await archives()).filter((a) => a.kinds["NCGR"]);
      arc = choices[0]?.id ?? null;
    }
    if (arc !== null) await load(arc);
  });

  const thumb = (i: Item) =>
    i.scr !== undefined
      ? asset(`screen/${arc}:${i.chr}/${arc}:${i.pal}/${arc}:${i.scr}`, version)
      : i.pal === null
      ? ""
      : asset(
          `preview/${arc}:${i.chr}/${arc}:${i.pal}?cols=8&paloffset=${i.offset}` +
            (i.cells === undefined ? "" : `&cells=${i.cells ?? "none"}`),
          version,
        );
  const changed = (chr: number) => changedSet.has(chr);

  const chr = $derived<Ref | null>(arc !== null && selected ? [arc, selected.chr] : null);
  const pal = $derived<Ref | null>(arc !== null && palette !== null ? [arc, palette] : null);
</script>

<div class="browser">
  <aside class="list">
    {#if archive === null}
      <label class="pick">
        <span class="muted">Archive</span>
        <select
          value={arc}
          onchange={(e) => {
            arc = Number((e.currentTarget as HTMLSelectElement).value);
            play("select");
            load(arc);
          }}
        >
          {#each choices as a}
            <option value={a.id}>{a.path} ({a.kinds["NCGR"]} sheets)</option>
          {/each}
        </select>
      </label>
    {/if}
    <input class="search" placeholder="Search {items.length}" bind:value={filter} />
    <VirtualList items={shown} height={54} key={(i) => i.key} label="Pictures">
      {#snippet row(item)}
        <button class="row-item sheet" role="option" aria-selected={item === selected} onclick={() => choose(item)}>
          <span class="thumb"><Thumb src={thumb(item)} /></span>
          <span class="label">{item.label}</span>
          {#if someAnimated && animated(item)}<span class="tag">animated</span>{/if}
          {#if changed(item.chr)}<span class="tag changed">changed</span>{/if}
        </button>
      {/snippet}
      {#snippet empty()}
        <p class="muted empty">{error ?? (items.length ? "Nothing matches." : "Loading...")}</p>
      {/snippet}
    </VirtualList>
  </aside>

  <section class="editor">
    {#if chr && pal && selected}
      {#if selected.scr !== undefined && arc !== null}
        <div class="screen-view window">
          <img class="px" src={asset(`screen/${arc}:${selected.chr}/${arc}:${pal[1]}/${arc}:${selected.scr}`, version)} alt="Screen {selected.scr}" />
          <p class="muted">Screen #{selected.scr}, tiles #{selected.chr}</p>
        </div>
      {/if}
      {#if selected.rig && arc !== null}
        {@const [big, cer, anr, mcr] = selected.rig}
        {#key `${big}/${pal[1]}/${version}`}
          <RigPlayer
            path={`${arc}:${big}/${arc}:${pal[1]}/${arc}:${cer}/${arc}:${anr}/${arc}:${mcr}`}
            onFrame={(f) => (rigFrameIndex = f)}
          >
            <div class="mode" role="radiogroup" aria-label="Edit">
              <button class:primary={editMode === "picture"} onclick={() => ((editMode = "picture"), play("select"))}>Picture</button>
              <button class:primary={editMode === "animation"} onclick={() => ((editMode = "animation"), play("select"))}>Animation</button>
            </div>
          </RigPlayer>
        {/key}
      {/if}
      {#key `${chr.join(":")}/${pal.join(":")}/${version}/${editMode}`}
        {@const rig = editMode === "animation" && selected.rig && arc !== null ? selected.rig : null}
        <PixelEditor
          chr={rig ? [arc!, rig[0]] : chr}
          rigFrame={rig ? { cer: [arc!, rig[1]], anr: [arc!, rig[2]], mcr: [arc!, rig[3]], frame: rigFrameIndex } : null}
          {pal}
          {readOnly}
          changed={changed(selected.chr)}
          palettes={selected.palettes.length ? selected.palettes : palettes.map((p) => [`#${p.index}`, p.index] as [string, number])}
          paletteOffset={selected.offset}
          cells={selected.cells}
          anims={selected.anims}
          onPalette={(i) => (palette = i)}
          {onSaved}
        />
      {/key}
    {:else if chr}
      <p class="muted pad">No palette for this sheet.</p>
    {:else}
      <p class="muted pad">Choose a picture.</p>
    {/if}
  </section>
</div>

<style>
  .browser {
    display: grid;
    grid-template-columns: 17rem minmax(0, 1fr);
    height: 100%;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.75rem;
    min-height: 0;
    border-right: 2px solid var(--outline);
  }
  .pick {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .sheet .thumb {
    width: 64px;
    height: 48px;
    display: grid;
    place-items: center;
    background: var(--bg);
    border-radius: 4px;
    overflow: hidden;
    flex: none;
  }
  .sheet .thumb :global(img) {
    max-width: 64px;
    max-height: 48px;
  }
  .empty,
  .pad {
    padding: 1rem;
  }
  .editor {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .mode {
    display: flex;
    gap: 0.25rem;
    margin-left: auto;
  }
  .mode button {
    padding: 0.2rem 0.7rem;
  }
  .screen-view {
    display: flex;
    align-items: flex-end;
    gap: 1rem;
    margin: 0.4rem 0.9rem;
    padding: 0.4rem;
  }
  .screen-view img {
    height: 128px;
    width: auto;
    max-width: 60%;
    object-fit: contain;
    background: var(--bg);
  }
  .editor > :global(.pixel-editor) {
    flex: 1;
    min-height: 0;
    height: auto;
  }
</style>
