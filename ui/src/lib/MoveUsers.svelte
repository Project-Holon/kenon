<script lang="ts">
  // Who learns a move: by level, as an egg move, from a TM or HM, and the
  // trainers who use it (core/src/battle.rs move_users).
  import { invoke } from "@tauri-apps/api/core";
  import { speciesIcon } from "./icons.svelte";
  import { enumValues, tableEntries } from "./tables";

  let { id, version }: { id: number; version: number } = $props();

  interface Users {
    levelUp: { species: number; level: number }[];
    egg: number[];
    machine: string | null;
    machineSpecies: number[];
    tutor: boolean;
    tutorSpecies: number[];
    trainers: number[];
  }

  let users = $state<Users | null>(null);
  let species = $state<string[]>([]);
  let trainers = $state<string[]>([]);
  let showAll = $state<Record<string, boolean>>({});
  const SHORT = 24;

  $effect(() => {
    void version;
    const at = id;
    users = null;
    showAll = {};
    invoke<Users>("move_users", { id: at })
      .then((u) => {
        if (id === at) users = u;
      })
      .catch(() => {});
  });
  $effect(() => {
    invoke<string[]>("record_names")
      .then((s) => (species = s))
      .catch(() => enumValues("species").then((s) => (species = s)));
    tableEntries("trainers")
      .then((t) => (trainers = t.map((e) => e.name)))
      .catch(() => {});
  });

  // Trainers met more than once share a name: "Caitlin x2".
  const named = (list: number[]) => {
    const counts = new Map<string, number>();
    for (const t of list) {
      const n = trainers[t] ?? `Trainer ${t}`;
      counts.set(n, (counts.get(n) ?? 0) + 1);
    }
    return [...counts].map(([n, c]) => (c > 1 ? `${n} x${c}` : n)).join(", ");
  };
  const spName = (sp: number) => species[sp] || `Form record ${sp}`;
  const cut = <T,>(key: string, list: T[]) => (showAll[key] || list.length <= SHORT ? list : list.slice(0, SHORT));
</script>

<div class="window block users">
  <h3>Who learns it</h3>
  {#if !users}
    <p class="muted">Looking...</p>
  {:else}
    {#if !users.levelUp.length && !users.egg.length && !users.machineSpecies.length && !users.tutorSpecies.length}
      <p class="muted">No Pokemon learns this move. Add it to a learnset under Stats and moves.</p>
    {/if}
    {#if users.levelUp.length}
      <h4>By level ({users.levelUp.length})</h4>
      <div class="chips">
        {#each cut("level", users.levelUp) as u}
          <span class="chip">
            {#if speciesIcon(u.species)}<img class="px" src={speciesIcon(u.species)} alt="" />{/if}
            {spName(u.species)} <span class="muted">Lv. {u.level || "evolving"}</span>
          </span>
        {/each}
        {#if users.levelUp.length > SHORT && !showAll.level}
          <button class="quiet" onclick={() => (showAll.level = true)}>{users.levelUp.length - SHORT} more</button>
        {/if}
      </div>
    {/if}
    {#if users.egg.length}
      <h4>As an egg move ({users.egg.length})</h4>
      <div class="chips">
        {#each cut("egg", users.egg) as sp}
          <span class="chip">{#if speciesIcon(sp)}<img class="px" src={speciesIcon(sp)} alt="" />{/if}{spName(sp)}</span>
        {/each}
        {#if users.egg.length > SHORT && !showAll.egg}
          <button class="quiet" onclick={() => (showAll.egg = true)}>{users.egg.length - SHORT} more</button>
        {/if}
      </div>
    {/if}
    {#if users.machine}
      <h4>{users.machine} ({users.machineSpecies.length})</h4>
      <div class="chips">
        {#each cut("tm", users.machineSpecies) as sp}
          <span class="chip">{#if speciesIcon(sp)}<img class="px" src={speciesIcon(sp)} alt="" />{/if}{spName(sp)}</span>
        {/each}
        {#if users.machineSpecies.length > SHORT && !showAll.tm}
          <button class="quiet" onclick={() => (showAll.tm = true)}>{users.machineSpecies.length - SHORT} more</button>
        {/if}
      </div>
    {/if}
    {#if users.tutor}
      <h4>From a move tutor ({users.tutorSpecies.length})</h4>
      <div class="chips">
        {#each cut("tutor", users.tutorSpecies) as sp}
          <span class="chip">{#if speciesIcon(sp)}<img class="px" src={speciesIcon(sp)} alt="" />{/if}{spName(sp)}</span>
        {/each}
        {#if users.tutorSpecies.length > SHORT && !showAll.tutor}
          <button class="quiet" onclick={() => (showAll.tutor = true)}>{users.tutorSpecies.length - SHORT} more</button>
        {/if}
      </div>
    {/if}
    {#if users.trainers.length}
      <h4>Trainers who use it ({users.trainers.length})</h4>
      <p class="trainers">
        {named(users.trainers)}
      </p>
    {/if}
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
  .trainers {
    font-size: 0.9rem;
  }
</style>
