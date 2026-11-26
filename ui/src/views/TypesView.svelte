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
  import ContentDialog from "../lib/ContentDialog.svelte";
  import { toast } from "morphe/menu.svelte";

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
  // The chart as saved, and as edited (a click cycles a cell).
  let saved = $state<number[][]>([]);
  let count = $state(17);
  async function loadAll() {
    count = await invoke<number>("type_count");
    clearEnums();
    types = (await enumValues("type")).slice(0, count);
    saved = await invoke<number[][]>("type_chart");
    chart = saved.map((r) => [...r]);
  }
  $effect(() => {
    void version;
    loadAll().catch((e) => (error = String(e)));
  });
  const dirty = $derived(JSON.stringify(chart) !== JSON.stringify(saved));
  const NEXT: Record<number, number> = { 4: 8, 8: 2, 2: 0, 0: 4 };
  function cycle(a: number, d: number) {
    if (readOnly) return;
    play("select");
    chart = chart.map((r, i) => (i === a ? r.map((v, j) => (j === d ? NEXT[v] ?? 4 : v)) : r));
  }
  let busy = $state(false);
  async function save() {
    busy = true;
    try {
      await invoke("set_type_chart", { chart });
      await loadAll();
      onSaved();
      play("save");
      toast("Type chart saved");
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }
  // New and Remove open kenon's add and remove dialog, as for Pokemon,
  // moves, items and abilities. The dialog shows errors itself.
  let dialog = $state<"add" | "remove" | null>(null);
  async function addType(from: number, name: string) {
    const id = await invoke<number>("add_type", { name, from });
    await loadAll();
    onSaved();
    play("save");
    picked = id;
    toast(`Added ${types[id]}`);
  }
  async function removeType(id: number) {
    const name = types[id];
    const r = await invoke<{ updated: string[] }>("remove_type", { id });
    picked = null;
    await loadAll();
    onSaved();
    play("save");
    const n = r.updated.length;
    toast(`Removed ${name}${n ? `. ${n} record${n > 1 ? "s" : ""} now use Normal` : ""}`);
  }
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
      <div class="title">
        <h2>Type chart</h2>
        {#if !readOnly}
          <div class="acts">
            <button class="quiet" disabled={busy || dirty} title={dirty ? "Save or discard chart edits first." : ""} onclick={() => ((dialog = "add"), play("decide"))}>New type</button>
            <button class="quiet" disabled={!dirty || busy} onclick={() => (chart = saved.map((r) => [...r]))}>Discard</button>
            <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
          </div>
        {/if}
      </div>
      <p class="muted">
        Attacking type by row, defending type by column. 2: super effective, &frac12;: not very effective, 0: no effect.
        {#if !readOnly}Click a cell to cycle effectiveness.{/if}
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
                <td
                  class="cell v{v}"
                  class:cross={picked === a || picked === d}
                  class:edited={saved[a]?.[d] !== v}
                  class:editable={!readOnly}
                  onmouseenter={() => (hover = [a, d])}
                  onclick={() => cycle(a, d)}
                >{MARK[v] ?? v}</td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
      <!-- The hovered cell in words, under the chart; the line keeps its
           height so the page does not move. -->
      <p class="matchup">
        {#if hover}<strong>{types[hover[0]]}</strong> {WORD[chart[hover[0]][hover[1]]]} <strong>{types[hover[1]]}</strong>.{/if}
      </p>
    </div>
  </section>

  <aside class="side">
    {#if picked === null}
      <p class="muted pad">Select a type to view matchups and users.</p>
    {:else}
      <div class="pad">
        <div class="side-head">
          <h2>{types[picked]}</h2>
          {#if !readOnly && picked >= 17}
            <button class="quiet" disabled={busy || dirty} title={dirty ? "Save or discard chart edits first." : ""} onclick={() => ((dialog = "remove"), play("decide"))}>Remove type</button>
          {/if}
        </div>
        <TextPanel kind="types" id={picked} {readOnly} onSaved={() => (onSaved(), clearEnums(), enumValues("type").then((t) => (types = t.slice(0, count))))} />
        <div class="facts">
          <div><span class="muted">Super effective against</span><div class="row">{#each strong as t}{@render typeChip(t)}{:else}<span class="muted">nothing</span>{/each}</div></div>
          <div><span class="muted">Weak to</span><div class="row">{#each weakTo as t}{@render typeChip(t)}{:else}<span class="muted">nothing</span>{/each}</div></div>
          <div><span class="muted">Resists</span><div class="row">{#each resists as t}{@render typeChip(t)}{:else}<span class="muted">nothing</span>{/each}</div></div>
          <div><span class="muted">Immune to</span><div class="row">{#each immune as t}{@render typeChip(t)}{:else}<span class="muted">nothing</span>{/each}</div></div>
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

{#if dialog}
  <ContentDialog
    mode={dialog}
    noun="type"
    names={types}
    from={picked ?? 0}
    fromLabel="Copy matchups from"
    removing={picked !== null && picked >= 17 ? { id: picked, name: types[picked] } : null}
    usesNote="Pokémon, moves and items of this type become Normal."
    icon={(t) => typeIcon(t) || null}
    onAdd={addType}
    onRemove={removeType}
    onClose={() => (dialog = null)}
  />
{/if}

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
  .title {
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .title h2 {
    flex: 1;
  }
  .acts {
    display: flex;
    gap: 0.5rem;
  }
  .side-head {
    display: flex;
    align-items: center;
    gap: 0.8rem;
  }
  .side-head h2 {
    flex: 1;
  }
  .cell.editable {
    cursor: pointer;
  }
  .cell.edited {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
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
  .matchup {
    min-height: 1.5em;
    margin: 0.6rem 0 0;
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
