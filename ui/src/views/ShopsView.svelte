<script lang="ts">
  // Shops (core shops.rs; eidos shops.txt): the 26 Poke Mart lists and the
  // 3 Battle Subway prize lists. A mart item sells at its own item price
  // (set on the item); a prize has its own BP price. At most 40 a list.
  import { invoke } from "@tauri-apps/api/core";
  import SearchSelect from "morphe/SearchSelect.svelte";
  import { toast } from "morphe/menu.svelte";
  import { enumValues } from "../lib/tables";
  import { itemIcon, loadIcons } from "../lib/icons.svelte";
  import { play } from "../lib/sfx";

  let { version, readOnly, onSaved }: { version: number; readOnly: boolean; onSaved: () => void } = $props();

  interface Shops {
    marts: number[][];
    prizes: [number, number][][];
  }
  const MAX = 40;
  // The main clerk's list by badges (eidos: the tier map in ov021).
  const BADGES = ["no badges", "1-2 badges", "3-4 badges", "5-6 badges", "7 badges", "8 badges"];
  const label = (kind: "mart" | "bp", n: number) =>
    kind === "bp" ? `Battle Subway prizes ${n + 1}` : n < 6 ? `Poke Mart, ${BADGES[n]}` : `Other clerk ${n - 5}`;

  let shops = $state<Shops | null>(null);
  let game = $state<Shops | null>(null);
  let original = $state("");
  let items = $state<string[]>([]);
  let prices = $state<number[]>([]);
  let current = $state<{ kind: "mart" | "bp"; n: number }>({ kind: "mart", n: 0 });
  let saving = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    void version;
    loadIcons();
    enumValues("item").then((i) => (items = i));
    invoke<number[]>("item_prices").then((p) => (prices = p)).catch(() => {});
    invoke<[Shops, Shops]>("shops")
      .then(([now, g]) => {
        shops = now;
        game = g;
        original = JSON.stringify(now);
      })
      .catch((e) => (error = String(e)));
  });

  const dirty = $derived(!!shops && JSON.stringify(shops) !== original);
  const changed = (kind: "mart" | "bp", n: number) =>
    !!shops && !!game && JSON.stringify(kind === "mart" ? shops.marts[n] : shops.prizes[n]) !== JSON.stringify(kind === "mart" ? game.marts[n] : game.prizes[n]);
  const count = $derived(shops ? (current.kind === "mart" ? shops.marts[current.n].length : shops.prizes[current.n].length) : 0);

  function add() {
    if (!shops || count >= MAX) return;
    if (current.kind === "mart") shops.marts[current.n].push(17);
    else shops.prizes[current.n].push([17, 1]);
  }
  function remove(i: number) {
    if (!shops) return;
    if (current.kind === "mart") shops.marts[current.n].splice(i, 1);
    else shops.prizes[current.n].splice(i, 1);
  }

  async function save() {
    if (!shops) return;
    saving = true;
    try {
      await invoke("set_shops", { shops });
      original = JSON.stringify(shops);
      play("save");
      toast("Saved");
      onSaved();
    } catch (e) {
      toast(String(e));
      play("cancel");
    } finally {
      saving = false;
    }
  }
</script>

{#snippet entry(kind: "mart" | "bp", n: number)}
  <button class="row-item" aria-current={current.kind === kind && current.n === n} onclick={() => ((current = { kind, n }), play("select"))}>
    <span class="label">{label(kind, n)}</span>
    {#if changed(kind, n)}<span class="tag changed">changed</span>{/if}
  </button>
{/snippet}

<div class="shops">
  <aside class="entries">
    {#if shops}
      <h4>Poke Marts</h4>
      {#each shops.marts as _, n}{@render entry("mart", n)}{/each}
      <h4>Battle Subway</h4>
      {#each shops.prizes as _, n}{@render entry("bp", n)}{/each}
    {/if}
  </aside>
  <section class="record">
    <div class="head entry-head">
      <div class="title">
        <h2>{label(current.kind, current.n)}</h2>
        <p class="muted">
          {count} of {MAX} items.
          {current.kind === "mart" ? "Each sells at its item's price." : "Prices in Battle Points."}
          {#if current.kind === "mart" && current.n >= 6}Clerks besides the main one pick a list in their script.{/if}
        </p>
      </div>
      {#if !readOnly}
        <div class="acts">
          <button disabled={!dirty || saving} onclick={() => shops && (shops = JSON.parse(original))}>Discard</button>
          <button class="primary" disabled={!dirty || saving} onclick={save}>{saving ? "Saving..." : "Save"}</button>
        </div>
      {/if}
    </div>
    {#if error}
      <p class="bad pad">{error}</p>
    {:else if shops}
      <div class="body">
        <div class="list">
          {#if current.kind === "mart"}
            {#each shops.marts[current.n] as _, i (i)}
              <div class="item">
                <div class="pick"><SearchSelect bind:value={shops.marts[current.n][i]} options={items} icon={itemIcon} disabled={readOnly} label="Item {i + 1}" /></div>
                <span class="price muted">{prices[shops.marts[current.n][i]] ?? ""}</span>
                {#if !readOnly}<button class="quiet x" onclick={() => remove(i)} aria-label="Remove item {i + 1}" title="Remove">×</button>{/if}
              </div>
            {/each}
          {:else}
            {#each shops.prizes[current.n] as row, i (i)}
              <div class="item">
                <div class="pick"><SearchSelect bind:value={row[0]} options={items} icon={itemIcon} disabled={readOnly} label="Prize {i + 1}" /></div>
                <input class="bp" type="number" min="1" max="65535" bind:value={row[1]} readonly={readOnly} aria-label="BP price {i + 1}" />
                {#if !readOnly}<button class="quiet x" onclick={() => remove(i)} aria-label="Remove prize {i + 1}" title="Remove">×</button>{/if}
              </div>
            {/each}
          {/if}
        </div>
        {#if !readOnly}
          <button class="addbtn" disabled={count >= MAX} onclick={add}>Add item</button>
        {/if}
      </div>
    {/if}
  </section>
</div>

<style>
  .shops {
    display: grid;
    grid-template-columns: 17rem minmax(0, 1fr);
    height: 100%;
  }
  .entries {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    padding: 0.75rem 0.5rem;
    overflow-y: auto;
    border-right: 2px solid var(--outline);
  }
  .entries h4 {
    margin: 0.6rem 0.5rem 0.3rem;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .row-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    text-align: left;
    background: none;
    border: none;
    box-shadow: none;
    font-weight: 500;
  }
  .row-item[aria-current="true"] {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .row-item .label {
    flex: 1;
  }
  .record {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.6rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .head .title {
    flex: 1;
  }
  .head h2 {
    margin: 0;
  }
  .head p {
    margin: 0.2rem 0 0;
    font-size: 0.85rem;
  }
  .acts {
    display: flex;
    gap: 0.5rem;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0.9rem 1rem;
  }
  .list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(21rem, 1fr));
    gap: 0.35rem 1.4rem;
  }
  .item {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 4.5rem 1.8rem;
    align-items: center;
    gap: 0.5rem;
  }
  .pick {
    min-width: 0;
  }
  .price {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .bp {
    width: 4.5rem;
  }
  .x {
    padding: 0.1rem 0.4rem;
    font-size: 1.1rem;
    line-height: 1;
  }
  .addbtn {
    margin-top: 0.8rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
