<script lang="ts">
  // An added type's pictures (core type_pictures.rs, eidos types.txt): its
  // icon, in the game's letters, the two colours of the icon and labels, and
  // the battle button shades that follow the main colour. Remake builds the
  // icon again from a game type's; Import takes a 32 x 16 PNG.
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { typeIcon } from "./icons.svelte";
  import { play } from "./sfx";
  import { toast } from "morphe/menu.svelte";

  let {
    id,
    types,
    readOnly,
    version,
    onChanged,
  }: { id: number; types: string[]; readOnly: boolean; version: number; onChanged: () => void } = $props();

  interface Look {
    icon: number;
    colors: string[];
    label: number | null;
    button: string[];
    hof: [number, number];
    color: string;
  }
  let look = $state<Look | null>(null);
  let fill = $state("#000000");
  let border = $state("#000000");
  let from = $state(0);
  let busy = $state(false);
  async function load() {
    look = await invoke<Look | null>("type_look", { id });
    fill = `#${look?.colors[0] ?? "000000"}`;
    border = `#${look?.colors[1] ?? "000000"}`;
  }
  $effect(() => {
    void id;
    void version;
    load().catch(() => (look = null));
  });

  async function run(what: () => Promise<unknown>, done: string) {
    busy = true;
    try {
      await what();
      await load();
      onChanged();
      play("save");
      toast(done);
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }
  const hex = (c: string) => c.replace("#", "").toUpperCase();
  function saveColors() {
    run(() => invoke("set_type_colors", { id, fill: hex(fill), border: hex(border) }), "Colors saved");
  }
  function remake() {
    run(() => invoke("remake_type_pictures", { id, from }), `Icon rebuilt from ${types[from]}`);
  }
  async function importIcon() {
    const path = await open({ title: "Choose a 32 x 16 PNG", filters: [{ name: "PNG image", extensions: ["png"] }] });
    if (typeof path === "string") run(() => invoke("import_type_icon", { id, path }), "Icon replaced");
  }
</script>

<section class="pictures">
  <h4>Pictures</h4>
  {#if !look}
    <p class="muted">No pictures yet.</p>
    {#if !readOnly}
      <div class="row">
        <select bind:value={from} aria-label="Make from">{#each types.slice(0, 17) as t, i}<option value={i}>{t}</option>{/each}</select>
        <button disabled={busy} onclick={remake}>Make pictures</button>
      </div>
    {/if}
  {:else}
    <div class="icon-row">
      <img class="big px" src={typeIcon(id)} alt="{types[id]} icon" />
      <div class="ramp" title="Battle move button shades">
        {#each look.button as c}<span style:background="#{c}"></span>{/each}
      </div>
    </div>
    {#if !readOnly}
      <div class="row">
        <label class="field"><span class="name">Main color</span><input type="color" bind:value={fill} /></label>
        <label class="field"><span class="name">Border color</span><input type="color" bind:value={border} /></label>
        <button disabled={busy || (hex(fill) === look.colors[0] && hex(border) === look.colors[1])} onclick={saveColors}>Save colors</button>
      </div>
      <div class="row">
        <label class="field">
          <span class="name">Rebuild icon from</span>
          <select bind:value={from}>{#each types.slice(0, 17) as t, i}<option value={i}>{t}</option>{/each}</select>
        </label>
        <button disabled={busy} onclick={remake}>Rebuild</button>
        <button disabled={busy} onclick={importIcon} title="32 x 16 pixels">Import PNG...</button>
      </div>
    {/if}
  {/if}
</section>

<style>
  .pictures {
    margin-top: 1rem;
    padding-top: 0.8rem;
    border-top: 1px solid var(--rim);
  }
  h4 {
    margin: 0 0 0.6rem;
  }
  .icon-row {
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .big {
    width: 128px;
    height: 64px;
    image-rendering: pixelated;
  }
  .ramp {
    display: flex;
    border: 1px solid var(--rim);
    border-radius: var(--radius-s);
    overflow: hidden;
  }
  .ramp span {
    width: 1.2rem;
    height: 1.6rem;
  }
  .row {
    display: flex;
    align-items: flex-end;
    flex-wrap: wrap;
    gap: 0.6rem 0.8rem;
    margin-top: 0.7rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .field .name {
    font-size: 0.85rem;
    color: var(--muted);
  }
  input[type="color"] {
    width: 4rem;
    height: 2rem;
    padding: 0;
  }
</style>
