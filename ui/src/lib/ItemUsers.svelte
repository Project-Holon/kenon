<script lang="ts">
  // Where an item is found or used: wild Pokemon that hold it, trainers
  // that use, hold or give it, trades, item balls and hidden items
  // (core/src/battle.rs item_users).
  import { invoke } from "@tauri-apps/api/core";
  import { speciesIcon } from "./icons.svelte";
  import { tableEntries } from "./tables";

  let { id, version }: { id: number; version: number } = $props();

  interface Users {
    wild: [number, number][];
    trainerBag: number[];
    trainerHeld: number[];
    trainerGift: number[];
    trades: number[];
    onMaps: [string, string][];
  }

  let users = $state<Users | null>(null);
  let species = $state<string[]>([]);
  let trainers = $state<string[]>([]);
  const SLOT = ["50%", "5%", "1%, shaking grass"];

  $effect(() => {
    void version;
    const at = id;
    users = null;
    invoke<Users>("item_users", { id: at })
      .then((u) => {
        if (id === at) users = u;
      })
      .catch(() => {});
  });
  $effect(() => {
    invoke<string[]>("record_names").then((s) => (species = s)).catch(() => {});
    tableEntries("trainers")
      .then((t) => (trainers = t.map((e) => e.name)))
      .catch(() => {});
  });

  // Trainers met more than once share a name: "Cheren x9".
  const tr = (list: number[]) => {
    const counts = new Map<string, number>();
    for (const t of list) {
      const n = trainers[t] ?? `Trainer ${t}`;
      counts.set(n, (counts.get(n) ?? 0) + 1);
    }
    return [...counts].map(([n, c]) => (c > 1 ? `${n} x${c}` : n)).join(", ");
  };
  const places = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const [place, kind] of users?.onMaps ?? []) {
      const key = `${place}${kind === "hidden" ? " (hidden)" : ""}`;
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return [...counts].map(([k, n]) => (n > 1 ? `${k} x${n}` : k));
  });
  const nothing = $derived(
    !!users &&
      !users.wild.length &&
      !users.trainerBag.length &&
      !users.trainerHeld.length &&
      !users.trainerGift.length &&
      !users.trades.length &&
      !users.onMaps.length,
  );
</script>

<div class="window block users">
  <h3>Where to find it</h3>
  {#if !users}
    <p class="muted">Looking...</p>
  {:else if nothing}
    <p class="muted">No locations found (shops not included).</p>
  {:else}
    {#if places.length}
      <h4>On the maps ({users.onMaps.length})</h4>
      <p>{places.join(", ")}</p>
    {/if}
    {#if users.wild.length}
      <h4>Held by wild Pokemon ({users.wild.length})</h4>
      <div class="chips">
        {#each users.wild as [sp, slot]}
          <span class="chip">
            {#if speciesIcon(sp)}<img class="px" src={speciesIcon(sp)} alt="" />{/if}{species[sp] ?? sp}
            <span class="muted">{SLOT[slot]}</span>
          </span>
        {/each}
      </div>
    {/if}
    {#if users.trainerGift.length}
      <h4>Given by trainers after a battle ({users.trainerGift.length})</h4>
      <p>{tr(users.trainerGift)}</p>
    {/if}
    {#if users.trainerBag.length}
      <h4>Used by trainers in battle ({users.trainerBag.length})</h4>
      <p>{tr(users.trainerBag)}</p>
    {/if}
    {#if users.trainerHeld.length}
      <h4>Held by trainers' Pokemon ({users.trainerHeld.length})</h4>
      <p>{tr(users.trainerHeld)}</p>
    {/if}
    {#if users.trades.length}
      <h4>Held by a traded Pokemon ({users.trades.length})</h4>
      <p class="muted">In-game trades {users.trades.join(", ")}</p>
    {/if}
    <p class="muted note">Shops not included.</p>
  {/if}
</div>

<style>
  /* Its own padding: the table editor's .block style does not reach a child component. */
  .block {
    padding: 0.9rem 1rem;
  }
  h3 {
    margin: 0 0 0.4rem;
  }
  h4 {
    margin: 0.7rem 0 0.35rem;
    font-size: 0.85rem;
    color: var(--muted);
  }
  p {
    margin: 0;
    font-size: 0.9rem;
  }
  .note {
    margin-top: 0.7rem;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.1rem 0.5rem 0.1rem 0.2rem;
    border-radius: 999px;
    background: var(--panel-2);
    border: 1px solid var(--rim);
    font-size: 0.85rem;
  }
  .chip img {
    width: 32px;
    height: 32px;
    margin: -6px 0;
  }
</style>
