<script lang="ts">
  import { onMount } from "svelte";
  import { archives, asset, members, type ArchiveInfo, type MemberInfo, type Ref } from "../lib/backend";
  import { play } from "../lib/sfx";
  import PixelEditor from "./PixelEditor.svelte";

  let {
    archive,
    readOnly,
    version,
    onSaved,
  }: { archive: number | null; readOnly: boolean; version: number; onSaved: () => void } = $props();

  // The archive shown; fixed per category (the view is rebuilt on a new one).
  // svelte-ignore state_referenced_locally
  let arc = $state<number | null>(archive);
  let choices = $state<ArchiveInfo[]>([]);
  let list = $state<MemberInfo[]>([]);
  let selected = $state<number | null>(null);
  let palette = $state<number | null>(null);
  let filter = $state("");
  let error = $state<string | null>(null);

  const sheets = $derived(list.filter((m) => m.kind === "NCGR" && String(m.index).includes(filter.trim())));
  const palettes = $derived(list.filter((m) => m.kind === "NCLR"));

  // The palette a sheet is drawn with by default: the nearest palette before
  // it in the archive, else the nearest after (the grouping aletheia's
  // background.py found in this game's archives).
  function pairedPalette(index: number): number | null {
    let before: number | null = null;
    let after: number | null = null;
    for (const p of palettes) {
      if (p.index < index) before = p.index;
      else if (after === null) after = p.index;
    }
    return before ?? after;
  }

  async function load(id: number) {
    error = null;
    list = [];
    selected = null;
    try {
      list = await members(id);
      const first = list.find((m) => m.kind === "NCGR");
      if (first) choose(first.index, false);
    } catch (e) {
      error = String(e);
    }
  }

  function choose(index: number, sound = true) {
    if (sound) play("select");
    selected = index;
    palette = pairedPalette(index);
  }

  onMount(async () => {
    if (arc === null) {
      choices = (await archives()).filter((a) => a.kinds["NCGR"]);
      arc = choices[0]?.id ?? null;
    }
    if (arc !== null) await load(arc);
  });

  const thumb = (index: number) =>
    palette !== null || palettes.length > 0
      ? asset(`preview/${arc}:${index}/${arc}:${pairedPalette(index) ?? 0}?cols=8`, version)
      : "";

  const chr = $derived<Ref | null>(arc !== null && selected !== null ? [arc, selected] : null);
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
    <input class="search" placeholder="Filter by number" bind:value={filter} />
    <div class="scroll" role="listbox" aria-label="Sheets">
      {#each sheets as m (m.index)}
        <button class="row-item sheet" role="option" aria-selected={m.index === selected} onclick={() => choose(m.index)}>
          <span class="thumb"><img class="px" loading="lazy" src={thumb(m.index)} alt="" /></span>
          <span class="label">#{m.index}</span>
          {#if m.changed}<span class="tag changed">changed</span>{/if}
        </button>
      {:else}
        <p class="muted empty">{error ?? (list.length ? "No sheets match." : "Loading...")}</p>
      {/each}
    </div>
  </aside>

  <section class="editor">
    {#if chr && pal}
      {#key `${chr.join(":")}/${pal.join(":")}/${version}`}
        <PixelEditor
          {chr}
          {pal}
          {readOnly}
          changed={list.find((m) => m.index === selected)?.changed ?? false}
          palettes={palettes.map((p) => p.index)}
          onPalette={(i) => (palette = i)}
          {onSaved}
        />
      {/key}
    {:else if chr}
      <p class="muted pad">This archive has no palette for this sheet.</p>
    {:else}
      <p class="muted pad">Choose a sheet.</p>
    {/if}
  </section>
</div>

<style>
  .browser {
    display: grid;
    grid-template-columns: 16rem minmax(0, 1fr);
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
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
    flex-direction: column;
    gap: 2px;
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
  .sheet .thumb img {
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
  }
</style>
