<script lang="ts">
  // The type chart as the game has it (core/src/battle.rs): attacking type
  // down the side, defending type across the top. A type picked shows the
  // Pokemon and moves of that type.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { play } from "../lib/sfx";
  import { loadIcons, moveIcon, speciesIcon, typeIcon } from "../lib/icons.svelte";
  import { clearEnums, enumValues } from "../lib/tables";
  import TextPanel from "../lib/TextPanel.svelte";

  let { version, readOnly, onSaved }: { version: number; readOnly: boolean; onSaved: () => void } = $props();

  let chart = $state<number[][]>([]);
  let types = $state<string[]>([]);
  let species = $state<string[]>([]);
  let moves = $state<string[]>([]);
  let picked = $state<number | null>(null);
  let hover = $state<[number, number] | null>(null);
  let users = $state<{ species: number[]; moves: number[] } | null>(null);
  let error = $state<string | null>(null);

  onMount(() => {
    loadIcons();
    enumValues("species").then((s) => (species = s));
    enumValues("move").then((m) => (moves = m));
  });
  $effect(() => {
    void version;
    enumValues("type").then((t) => (types = t.slice(0, 17)));
    invoke<number[][]>("type_chart")
      .then((c) => (chart = c))
      .catch((e) => (error = String(e)));
  });
  $effect(() => {
    void version;
    const at = picked;
    users = null;
    if (at === null) return;
    invoke<{ species: number[]; moves: number[] }>("type_users", { id: at })
      .then((u) => {
        if (picked === at) users = u;
      })
      .catch(() => {});
  });

  const MARK: Record<number, string> = { 0: "0", 2: "½", 4: "", 8: "2" };
  const WORD: Record<number, string> = { 0: "has no effect on", 2: "is not very effective against", 4: "hits normally", 8: "is super effective against" };

  function pick(t: number) {
    play("select");
    picked = picked === t ? null : t;
  }

  // What a type does and takes, for the panel.
  const strong = $derived(picked === null || !chart[picked] ? [] : chart[picked].map((v, d) => [v, d]).filter(([v]) => v === 8).map(([, d]) => d));
  // The attacking types whose factor against the picked type is `factor`.
  function against(factor: number): number[] {
    const p = picked;
    if (p === null) return [];
    return chart.map((row, a) => [row[p], a]).filter(([v]) => v === factor).map(([, a]) => a);
  }
  const weakTo = $derived(against(8));
  const resists = $derived(against(2));
  const immune = $derived(against(0));
</script>

{#snippet typeChip(t: number)}
  <button class="tchip" onclick={() => pick(t)} title={types[t]}>
    {#if typeIcon(t)}<img class="px" src={typeIcon(t)} alt={types[t]} />{:else}{types[t]}{/if}
  </button>
{/snippet}

<div class="types">
  <section class="chart-pane">
    <div class="head">
      <h2>Type chart</h2>
      <p class="muted">
        Attacking type down the side, defending type across the top. 2: super effective, &frac12;: not very effective, 0: no effect.
        {#if hover}<br /><strong>{types[hover[0]]}</strong> {WORD[chart[hover[0]][hover[1]]]} <strong>{types[hover[1]]}</strong>.{/if}
      </p>
    </div>
    {#if error}<p class="bad pad">{error}</p>{/if}
    <div class="scroll">
      <table class="chart" onmouseleave={() => (hover = null)}>
        <thead>
          <tr>
            <th></th>
            {#each types as _, d}
              <th class="col" class:on={picked === d}>
                <button class="icon" onclick={() => pick(d)} title={types[d]}>
                  {#if typeIcon(d)}<img class="px" src={typeIcon(d)} alt={types[d]} />{:else}{types[d]}{/if}
                </button>
              </th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each chart as row, a}
            <tr class:on={picked === a}>
              <th class="rowhead">
                <button class="icon" onclick={() => pick(a)} title={types[a]}>
                  {#if typeIcon(a)}<img class="px" src={typeIcon(a)} alt={types[a]} />{:else}{types[a]}{/if}
                </button>
              </th>
              {#each row as v, d}
                <td class="cell v{v}" class:cross={picked === a || picked === d} onmouseenter={() => (hover = [a, d])}>{MARK[v] ?? v}</td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="muted note">
        This is the game's own chart. Changing it, or adding a type, needs support in eidos first (a chart a mod can give, and
        room for more types); kenon will edit it here then.
      </p>
    </div>
  </section>

  <aside class="side">
    {#if picked === null}
      <p class="muted pad">Pick a type to see what it is strong and weak against, and who uses it.</p>
    {:else}
      <div class="pad">
        <h2>{types[picked]}</h2>
        <TextPanel kind="types" id={picked} {readOnly} onSaved={() => (onSaved(), clearEnums(), enumValues("type").then((t) => (types = t.slice(0, 17))))} />
        <div class="facts">
          <div><span class="muted">Super effective against</span><div class="row">{#each strong as t}{@render typeChip(t)}{:else}<span class="muted">nothing</span>{/each}</div></div>
          <div><span class="muted">Weak to</span><div class="row">{#each weakTo as t}{@render typeChip(t)}{:else}<span class="muted">nothing</span>{/each}</div></div>
          <div><span class="muted">Resists</span><div class="row">{#each resists as t}{@render typeChip(t)}{:else}<span class="muted">nothing</span>{/each}</div></div>
          <div><span class="muted">Not affected by</span><div class="row">{#each immune as t}{@render typeChip(t)}{:else}<span class="muted">nothing</span>{/each}</div></div>
        </div>
        {#if !users}
          <p class="muted">Looking...</p>
        {:else}
          <h4>Pokemon ({users.species.length})</h4>
          <div class="chips">
            {#each users.species as sp}
              <span class="chip">{#if speciesIcon(sp)}<img class="px" src={speciesIcon(sp)} alt="" />{/if}{species[sp] ?? sp}</span>
            {/each}
          </div>
          <h4>Moves ({users.moves.length})</h4>
          <div class="chips">
            {#each users.moves as mv}
              <span class="chip">{#if moveIcon(mv)}<img class="px small" src={moveIcon(mv)} alt="" />{/if}{moves[mv] ?? mv}</span>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </aside>
</div>

<style>
  .types {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 22rem;
    height: 100%;
    min-height: 0;
  }
  .chart-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
  }
  .head {
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--outline);
    background: var(--bg);
  }
  .scroll {
    overflow: auto;
    padding: 1rem;
    min-height: 0;
  }
  .side {
    border-left: 2px solid var(--outline);
    overflow-y: auto;
    min-height: 0;
  }
  .pad {
    padding: 1rem;
  }
  .chart {
    border-collapse: collapse;
  }
  .chart th,
  .chart td {
    padding: 0;
  }
  .icon {
    background: none;
    border: none;
    box-shadow: none;
    padding: 2px;
    display: grid;
    place-items: center;
  }
  .icon img {
    height: 16px;
    image-rendering: pixelated;
  }
  .cell {
    width: 2.3rem;
    height: 1.9rem;
    text-align: center;
    font-weight: 700;
    border: 1px solid var(--rim);
    cursor: default;
  }
  .cell.v8 {
    background: color-mix(in srgb, var(--ok) 55%, transparent);
  }
  .cell.v2 {
    background: color-mix(in srgb, var(--bad) 40%, transparent);
  }
  .cell.v0 {
    background: var(--outline);
    color: var(--muted);
  }
  .cell.cross {
    outline: 2px solid var(--gold);
    outline-offset: -2px;
  }
  tr.on .rowhead,
  th.col.on {
    background: var(--panel-3);
  }
  .note {
    margin-top: 1rem;
    font-size: 0.9rem;
    max-width: 48rem;
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    margin: 0.75rem 0;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin-top: 0.2rem;
  }
  .tchip {
    background: none;
    border: none;
    box-shadow: none;
    padding: 0;
  }
  .tchip img {
    height: 16px;
  }
  h4 {
    margin: 0.9rem 0 0.35rem;
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
  .chip img.small {
    width: auto;
    height: 14px;
    margin: 0;
  }
</style>
