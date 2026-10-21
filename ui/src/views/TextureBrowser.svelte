<script lang="ts">
  import { onMount } from "svelte";
  import { asset } from "../lib/backend";
  import { once, debounced } from "../lib/cache.svelte";
  import { play } from "../lib/sfx";
  import VirtualList from "../lib/VirtualList.svelte";
  import Thumb from "../lib/Thumb.svelte";
  import { invoke } from "@tauri-apps/api/core";

  // The 3D textures of an archive: the overworld's ground, buildings,
  // trees and characters are 3D, and these textures are their look.
  let { archive, noun = "Set" }: { archive: number; noun?: string } = $props();

  interface Texture {
    block: number;
    index: number;
    name: string;
    width: number;
    height: number;
    format: string;
  }

  let count = $state(0);
  let current = $state<number | null>(null);
  let textures = $state<Texture[]>([]);
  let big = $state<Texture | null>(null);
  let filter = $state("");
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      const members = await once(`count/${archive}`, () => invoke<{ index: number }[]>("members", { arc: archive }));
      count = members.length;
      if (count) await open(0, false);
    } catch (e) {
      error = String(e);
    }
  });

  async function open(index: number, sound = true) {
    if (sound) play("select");
    current = index;
    big = null;
    textures = [];
    try {
      textures = await once(`textures/${archive}:${index}`, () =>
        fetch(asset(`textures/${archive}:${index}`)).then((r) => r.json()),
      );
    } catch (e) {
      error = String(e);
    }
  }

  const sets = $derived(Array.from({ length: count }, (_, i) => i));
  const query = debounced(() => filter);
  const shown = $derived.by(() => {
    const q = query.value.trim().toLowerCase();
    return q ? textures.filter((t) => t.name.toLowerCase().includes(q)) : textures;
  });
  const src = (t: Texture) => asset(`texture/${archive}:${current}/${t.block}/${t.index}`);
</script>

<div class="textures">
  <aside class="sets">
    <VirtualList items={sets} height={34} key={(i) => i} label="Texture sets">
      {#snippet row(i)}
        <button class="row-item" aria-current={i === current} onclick={() => open(i)}>
          <span class="label">{noun} {i}</span>
        </button>
      {/snippet}
      {#snippet empty()}
        <p class="muted pad">{error ?? "Loading..."}</p>
      {/snippet}
    </VirtualList>
  </aside>

  <section class="body">
    <div class="head">
      <input class="search" placeholder="Search {textures.length} textures" bind:value={filter} />
      <p class="muted small">Viewing only for now; editing 3D textures comes with the map editor.</p>
    </div>
    {#if big}
      <div class="window zoom">
        <img class="px" src={src(big)} alt={big.name} style="width: {Math.min(512, big.width * 4)}px" />
        <div>
          <strong>{big.name}</strong>
          <p class="muted">{big.width} x {big.height}, {big.format}</p>
          <button onclick={() => ((big = null), play("cancel"))}>Close</button>
        </div>
      </div>
    {/if}
    <div class="grid">
      {#each shown as t (`${t.block}/${t.index}`)}
        <button class="tile window" onclick={() => ((big = t), play("decide"))} title="{t.name}, {t.width}x{t.height}, {t.format}">
          <span class="pic"><Thumb src={src(t)} /></span>
          <span class="name">{t.name}</span>
        </button>
      {:else}
        <p class="muted pad">{current === null ? "" : textures.length ? "Nothing matches." : "No textures in this set."}</p>
      {/each}
    </div>
  </section>
</div>

<style>
  .textures {
    display: grid;
    grid-template-columns: 12rem minmax(0, 1fr);
    height: 100%;
  }
  .sets {
    display: flex;
    flex-direction: column;
    padding: 0.75rem 0.5rem;
    min-height: 0;
    border-right: 2px solid var(--outline);
  }
  .body {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .search {
    width: min(20rem, 100%);
  }
  .zoom {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
    margin: 0.75rem 1rem 0;
    padding: 0.75rem;
  }
  .zoom img {
    max-width: 100%;
    height: auto;
    image-rendering: pixelated;
    background: repeating-conic-gradient(#20242c 0 25%, #191c22 0 50%) 0 0 / 16px 16px;
  }
  .grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(7rem, 1fr));
    align-content: start;
    gap: 0.5rem;
    padding: 1rem;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    padding: 0.5rem;
  }
  .pic {
    width: 96px;
    height: 96px;
    display: grid;
    place-items: center;
    background: repeating-conic-gradient(#20242c 0 25%, #191c22 0 50%) 0 0 / 12px 12px;
    border-radius: 4px;
  }
  .pic :global(img) {
    max-width: 96px;
    max-height: 96px;
    image-rendering: pixelated;
  }
  .name {
    font-size: 0.8rem;
    font-weight: 500;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .small {
    font-size: 0.85rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
