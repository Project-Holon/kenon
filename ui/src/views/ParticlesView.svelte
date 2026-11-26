<script lang="ts">
  // Battle particle files (a/0/0/6, core spa.rs): each emitter's fields in
  // cards by group, optional parts (animations, children, behaviors)
  // switched on and off, and the textures, replaced from PNG files. A
  // script's LoadParticles names a file by its number.
  import { invoke } from "@tauri-apps/api/core";
  import { open as openFile } from "@tauri-apps/plugin-dialog";
  import { asset } from "../lib/backend";
  import { enumValues } from "../lib/tables";
  import { play } from "../lib/sfx";
  import { toast } from "morphe/menu.svelte";
  import VirtualList from "../lib/VirtualList.svelte";

  let { readOnly, version, onSaved }: { readOnly: boolean; version: number; onSaved: () => void } = $props();

  interface Listed {
    index: number;
    emitters: number;
    textures: number;
    changed: boolean;
    moves: number[];
  }
  interface Field {
    block: string;
    key: string;
    label: string;
    group: string;
    width: number;
    kind: "uint" | "int" | "fixed" | "angle" | "color" | "choice" | "flag";
    choices: string[];
    note: string;
  }
  interface TexInfo {
    format: number;
    width: number;
    height: number;
    colors: number;
  }
  interface FileView {
    emitters: { values: Record<string, number>; blocks: string[] }[];
    textures: TexInfo[];
    problems: string[];
  }
  const FORMATS = ["none", "A3I5", "4 colors", "16 colors", "256 colors", "4x4", "A5I3", "direct"];
  // The group each optional part fills.
  const BLOCK_OF: Record<string, string> = {
    "Size animation": "scaleAnim",
    "Color animation": "colorAnim",
    "Opacity animation": "alphaAnim",
    "Texture animation": "texAnim",
    Children: "child",
    Gravity: "gravity",
    "Random force": "random",
    Magnet: "magnet",
    Orbit: "spin",
    Floor: "collision",
    Pull: "convergence",
  };

  let list = $state<Listed[]>([]);
  let fields = $state<Field[]>([]);
  let moveNames = $state<string[]>([]);
  let current = $state<number | null>(null);
  let file = $state<FileView | null>(null);
  let emitter = $state(0);
  let filter = $state("");
  let busy = $state(false);
  // Edits: values and blocks, per emitter, until saved.
  let values = $state<Record<string, number>[]>([]);
  let blocks = $state<string[][]>([]);

  async function loadList() {
    list = await invoke<Listed[]>("particle_list");
  }
  $effect(() => {
    void version;
    loadList().catch((e) => toast(String(e)));
    invoke<Field[]>("particle_fields").then((f) => (fields = f));
    enumValues("move").then((m) => (moveNames = m));
  });
  async function open(index: number) {
    if (dirty && current !== null) {
      toast("Save or discard first.");
      return;
    }
    current = index;
    file = await invoke<FileView>("particle_file", { index });
    values = file.emitters.map((e) => ({ ...e.values }));
    blocks = file.emitters.map((e) => [...e.blocks]);
    emitter = 0;
  }
  const dirty = $derived(
    !!file &&
      (JSON.stringify(values) !== JSON.stringify(file.emitters.map((e) => e.values)) ||
        JSON.stringify(blocks) !== JSON.stringify(file.emitters.map((e) => e.blocks))),
  );
  function discard() {
    if (current !== null) {
      const at = current;
      current = null;
      open(at);
    }
  }
  async function save() {
    if (!file || current === null) return;
    busy = true;
    try {
      const blockEdits: [number, string, boolean][] = [];
      blocks.forEach((now, e) => {
        const was = file!.emitters[e].blocks;
        for (const b of new Set([...now, ...was])) if (now.includes(b) !== was.includes(b)) blockEdits.push([e, b, now.includes(b)]);
      });
      const edits: { emitter: number; key: string; value: number }[] = [];
      values.forEach((now, e) => {
        for (const [k, v] of Object.entries(now)) if (file!.emitters[e].values[k] !== v) edits.push({ emitter: e, key: k, value: v });
      });
      await invoke("set_particle_file", { index: current, blocks: blockEdits, edits });
      const at = current;
      current = null;
      await open(at);
      await loadList();
      onSaved();
      play("save");
      toast(`Saved particles ${at}`);
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }
  async function copy() {
    if (current === null) return;
    const from = current;
    try {
      const at = await invoke<number>("add_particle_file", { from });
      await loadList();
      current = null;
      await open(at);
      onSaved();
      play("save");
      toast(`Created particles ${at} from ${from}`);
    } catch (e) {
      toast(String(e));
    }
  }
  async function importTexture(t: number) {
    if (current === null) return;
    const path = await openFile({ title: "Choose a PNG", filters: [{ name: "PNG image", extensions: ["png"] }] });
    if (typeof path !== "string") return;
    try {
      await invoke("import_particle_texture", { index: current, texture: t, path });
      const at = current;
      current = null;
      await open(at);
      onSaved();
      play("save");
      toast(`Replaced texture ${t + 1}`);
    } catch (e) {
      toast(String(e));
    }
  }

  const shown = $derived.by(() => {
    const f = filter.trim().toLowerCase();
    if (!f) return list;
    return list.filter((l) => String(l.index) === f || l.moves.some((m) => moveNames[m]?.toLowerCase().includes(f)));
  });
  const label = (l: Listed) =>
    l.moves.length ? l.moves.map((m) => moveNames[m] ?? `Move ${m}`).join(", ") : "Not used by any move";

  // Groups of the open emitter, in field order.
  const groups = $derived.by(() => {
    const out: { name: string; block: string | null; fields: Field[] }[] = [];
    for (const f of fields) {
      let g = out.find((x) => x.name === f.group);
      if (!g) out.push((g = { name: f.group, block: BLOCK_OF[f.group] ?? null, fields: [] }));
      g.fields.push(f);
    }
    return out;
  });
  const has = (b: string | null) => !b || blocks[emitter]?.includes(b);
  // Switching a part on or off saves at once (with the other edits), so a
  // new part's fields show with their starting values.
  function toggle(b: string, on: boolean) {
    blocks = blocks.map((list, e) => (e === emitter ? (on ? [...list, b] : list.filter((x) => x !== b)) : list));
    const keep = emitter;
    save().then(() => (emitter = keep));
  }
  const shownFields = (g: { fields: Field[] }) => g.fields.filter((f) => values[emitter]?.[f.key] !== undefined);
  // Units the editor shows.
  const toUi = (f: Field, v: number) =>
    f.kind === "fixed" ? +(v / 4096).toFixed(3) : f.kind === "angle" ? Math.round((v * 360) / 65536) : v;
  const fromUi = (f: Field, v: number) =>
    f.kind === "fixed" ? Math.round(v * 4096) : f.kind === "angle" ? Math.round((v * 65536) / 360) & 0xffff : Math.round(v);
  const rgb = (c: number) => {
    const ch = (s: number) => (((c >> s) & 31) * 255 / 31) | 0;
    return `#${[0, 5, 10].map((s) => ch(s).toString(16).padStart(2, "0")).join("")}`;
  };
  const from555 = (hex: string) => {
    const v = parseInt(hex.slice(1), 16);
    return ((v >> 19) & 31) | (((v >> 11) & 31) << 5) | (((v >> 3) & 31) << 10);
  };
  function set(f: Field, v: number) {
    values = values.map((vals, e) => (e === emitter ? { ...vals, [f.key]: v } : vals));
  }
</script>

<div class="particles">
  <aside class="files">
    <div class="list-head">
      <input class="search" placeholder="Search {list.length} files by move" bind:value={filter} />
    </div>
    <VirtualList items={shown} height={46} key={(l) => l.index} label="Particle files">
      {#snippet row(l)}
        <button class="row-item" class:on={l.index === current} onclick={() => open(l.index)}>
          <span class="num">{l.index}</span>
          <span class="who">{label(l)}</span>
          {#if l.changed}<span class="tag changed">changed</span>{/if}
        </button>
      {/snippet}
    </VirtualList>
  </aside>

  <section class="main">
    {#if !file || current === null}
      <p class="muted pad">Select a particle file. Move animations reference files by number.</p>
    {:else}
      <header class="head">
        <h2>Particles {current}</h2>
        <span class="muted used">{label(list.find((l) => l.index === current) ?? { index: 0, emitters: 0, textures: 0, changed: false, moves: [] })}</span>
        <div class="spacer"></div>
        {#if !readOnly}
          <button class="quiet" disabled={busy || dirty} onclick={copy} title={dirty ? "Save or discard first." : "Duplicate into a new file"}>Duplicate</button>
          <button class="quiet" disabled={!dirty || busy} onclick={discard}>Discard</button>
          <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
        {/if}
      </header>
      {#if file.problems.length}
        <p class="bad pad-x">{file.problems.join(" ")}</p>
      {/if}
      <div class="textures">
        {#each file.textures as t, i}
          <figure>
            <img class="px" src={asset(`spatex/${current}/${i}`, version)} alt="Texture {i + 1}" />
            {#if !readOnly}<button class="quiet" onclick={() => importTexture(i)}>Import PNG...</button>{/if}
            <figcaption>{i + 1}: {t.width}x{t.height} {FORMATS[t.format]}</figcaption>
          </figure>
        {/each}
      </div>
      <nav class="sec-tabs">
        {#each file.emitters as _, e}
          <button class:on={e === emitter} onclick={() => (emitter = e)}>Emitter {e + 1}</button>
        {/each}
      </nav>
      <div class="cards">
        {#each groups as g}
          {@const shownF = shownFields(g)}
          <div class="window card" class:off={!has(g.block)}>
            <div class="card-head">
              <h4>{g.name}</h4>
              {#if g.block && !readOnly}
                <label class="switch"><input type="checkbox" checked={has(g.block)} onchange={(ev) => toggle(g.block!, ev.currentTarget.checked)} /> On</label>
              {/if}
            </div>
            {#if has(g.block) && shownF.length}
              <div class="grid">
                {#each shownF as f}
                  {@const v = values[emitter][f.key]}
                  <span class="name" title={f.note}>{f.label}</span>
                  <span class="ctl">
                    {#if f.kind === "flag"}
                      <input type="checkbox" checked={v !== 0} disabled={readOnly} onchange={(ev) => set(f, ev.currentTarget.checked ? 1 : 0)} />
                    {:else if f.kind === "choice"}
                      <select value={String(v)} disabled={readOnly} onchange={(ev) => set(f, Number(ev.currentTarget.value))}>
                        {#each f.choices as c, i}<option value={String(i)}>{c}</option>{/each}
                      </select>
                    {:else if f.kind === "color"}
                      <input type="color" value={rgb(v)} disabled={readOnly} onchange={(ev) => set(f, from555(ev.currentTarget.value))} />
                    {:else}
                      <input
                        type="number"
                        step={f.kind === "fixed" ? 0.05 : 1}
                        value={toUi(f, v)}
                        disabled={readOnly}
                        onchange={(ev) => set(f, fromUi(f, Number(ev.currentTarget.value)))}
                      />
                    {/if}
                  </span>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .particles {
    display: grid;
    grid-template-columns: 20rem minmax(0, 1fr);
    height: 100%;
  }
  .files {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    border-right: 2px solid var(--outline);
    min-height: 0;
  }
  .row-item {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    height: 100%;
    padding: 0 0.8rem;
    text-align: left;
    background: none;
    border: none;
    box-shadow: none;
  }
  .row-item.on {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .num {
    font-weight: 700;
    min-width: 2.5rem;
  }
  .who {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
  }
  .main {
    overflow: auto;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .head h2 {
    margin: 0;
  }
  .used {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }
  .textures {
    display: flex;
    flex-wrap: wrap;
    gap: 0.8rem;
    padding: 0.8rem 1rem;
  }
  figure {
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    padding: 0.5rem;
    border: 1px solid var(--rim);
    border-radius: var(--radius-s);
    background: repeating-conic-gradient(#8884 0% 25%, transparent 0% 50%) 0 / 12px 12px;
  }
  figure img {
    width: 64px;
    height: 64px;
    object-fit: contain;
    image-rendering: pixelated;
  }
  figcaption {
    font-size: 0.8rem;
  }
  .sec-tabs {
    padding: 0 1rem;
  }
  .cards {
    columns: 18rem 3;
    column-gap: 1rem;
    padding: 0.8rem 1rem;
  }
  .card {
    break-inside: avoid;
    margin-bottom: 1rem;
    padding: 0.7rem 0.9rem;
  }
  .card.off h4 {
    color: var(--muted);
  }
  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .card-head h4 {
    margin: 0;
  }
  .switch {
    font-size: 0.85rem;
    display: flex;
    gap: 0.3rem;
    align-items: center;
  }
  .grid {
    display: grid;
    grid-template-columns: minmax(5rem, max-content) minmax(0, 1fr);
    gap: 0.35rem 0.7rem;
    align-items: center;
    margin-top: 0.6rem;
  }
  .grid .name {
    font-size: 0.85rem;
    color: var(--muted);
  }
  .ctl input[type="number"],
  .ctl select {
    width: 100%;
  }
  .pad {
    padding: 1rem;
  }
  .pad-x {
    padding: 0.5rem 1rem 0;
    margin: 0;
  }
  .bad {
    color: var(--bad);
  }
</style>
