<script lang="ts">
  // Every picture of one Pokemon (or form slot) in one place: battle
  // sprites (a/0/0/4, 20 members per slot), party icons (a/0/0/7, 7 + 2n
  // and the female one after it) and the Pokedex icon (a/1/6/7, n + 5).
  // Each opens in its picture screen, where it is edited or replaced.
  import { asset, namedSheets, type NamedSheet } from "./backend";
  import { picturePath } from "./graphicsMenu";
  import { play } from "./sfx";

  let { slot, version }: { slot: number; version: number } = $props();

  interface Shown {
    label: string;
    path: string;
    category: string;
    chr: number;
  }
  const ARCHIVES: [number, string, (chr: number, n: number) => boolean][] = [
    [4, "battle-sprites", (chr, n) => chr >= 20 * n && chr < 20 * n + 20],
    [7, "pokemon-icons", (chr, n) => chr === 7 + 2 * n || chr === 8 + 2 * n],
    [167, "dex-icons", (chr, n) => chr === n + 5],
  ];

  let shown = $state<Shown[]>([]);
  $effect(() => {
    const n = slot;
    void version;
    Promise.all(ARCHIVES.map(([arc]) => namedSheets(arc))).then((lists) => {
      if (n !== slot) return;
      const out: Shown[] = [];
      lists.forEach((list, k) => {
        const [arc, category, mine] = ARCHIVES[k];
        for (const s of (list ?? []).filter((s: NamedSheet) => mine(s.chr, n))) {
          const pic = (pal: number) =>
            picturePath({ arc, label: s.label, chr: s.chr, pal, offset: s.paletteOffset, cells: s.cells, anims: s.anims, rig: s.rig });
          out.push({ label: s.label, path: pic(s.pal), category, chr: s.chr });
          for (const [name, pal] of s.palettes) {
            out.push({ label: `${s.label} (${name.toLowerCase()})`, path: pic(pal), category, chr: s.chr });
          }
        }
      });
      shown = out;
    });
  });
  function openIn(p: Shown) {
    play("decide");
    window.dispatchEvent(new CustomEvent("kenon:open", { detail: { category: p.category, index: p.chr } }));
  }
</script>

<div class="window block">
  <h3>Pictures</h3>
  {#if !shown.length}
    <p class="muted">No pictures found.</p>
  {:else}
    <div class="grid">
      {#each shown as p}
        <button class="pic" onclick={() => openIn(p)} title="Open to edit or replace">
          {#if p.path}<img class="px" src={asset(p.path, version)} alt={p.label} />{/if}
          <span>{p.label}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .block {
    padding: 0.9rem 1rem;
  }
  h3 {
    margin: 0 0 0.7rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
    gap: 0.8rem;
  }
  .pic {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.35rem;
    padding: 0.6rem;
    background: var(--bg);
    border: 1px solid var(--rim);
    border-radius: var(--radius-s);
    box-shadow: none;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
  }
  .pic img {
    max-width: 100%;
    height: 96px;
    object-fit: contain;
    image-rendering: pixelated;
  }
  .pic:hover {
    border-color: var(--accent);
    color: var(--text);
  }
</style>
