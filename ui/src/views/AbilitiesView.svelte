<script lang="ts">
  // Abilities: what each one says it does, the battle code that does it
  // (core/src/battle.rs), and the Pokemon that have it. An ability is all
  // code: the game finds it by the ability's id.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import VirtualList from "../lib/VirtualList.svelte";
  import { debounced } from "morphe/cache.svelte";
  import { play } from "../lib/sfx";
  import { loadIcons, speciesIcon } from "../lib/icons.svelte";
  import { clearEnums, enumValues } from "../lib/tables";
  import { textBank } from "../lib/backend";
  import { toast } from "morphe/menu.svelte";
  import ContentDialog from "../lib/ContentDialog.svelte";
  import TextPanel from "../lib/TextPanel.svelte";
  import BattleHandler from "../lib/BattleHandler.svelte";

  let { version, readOnly, onSaved }: { version: number; readOnly: boolean; onSaved: () => void } = $props();

  // New abilities (core/src/content.rs ABILITIES): a name, a description,
  // and later the battle code of another ability (eidos).
  let added = $state<{ last: number; ids: number[] } | null>(null);
  let dialog = $state<"add" | "remove" | null>(null);
  $effect(() => {
    void version;
    invoke<{ last: number; ids: number[] }>("content_added", { table: "abilities" })
      .then((a) => (added = a))
      .catch(() => (added = null));
  });
  const isAdded = (id: number) => !!added && id > added.last;

  async function reloadNames() {
    clearEnums();
    names = await enumValues("ability");
    descriptions = await textBank(2, 183);
  }
  async function addAbility(from: number, name: string) {
    const id = await invoke<number>("content_add", { table: "abilities", from, name });
    play("save");
    onSaved();
    await reloadNames();
    current = id;
    list?.reveal(shown.findIndex((e) => e.index === id));
    toast(`Added ${name} as ability ${id}`);
  }
  async function removeAbility(id: number) {
    const name = names[id];
    const r = await invoke<{ renumbered: [number, number][]; cleared: string[] }>("content_remove", { table: "abilities", id });
    play("save");
    onSaved();
    await reloadNames();
    current = Math.min(id, names.length - 1);
    const parts = [`Removed ${name}`];
    if (r.renumbered.length) parts.push(`${r.renumbered.length} later abilit${r.renumbered.length > 1 ? "ies" : "y"} moved down one number`);
    if (r.cleared.length) parts.push(`Pokemon that had it now have none: ${r.cleared.join(", ")}`);
    toast(parts.join("; "));
  }

  interface Coded {
    id: number;
    function: string | null;
    label: string | null;
    sharedWith: number[];
  }
  interface Users {
    first: number[];
    second: number[];
    hidden: number[];
  }

  let names = $state<string[]>([]);
  let descriptions = $state<string[]>([]);
  let species = $state<string[]>([]);
  let coded = $state<Coded[]>([]);
  let current = $state(1);
  let users = $state<Users | null>(null);
  let filter = $state("");
  let list = $state<{ reveal: (index: number) => void } | null>(null);

  onMount(() => {
    loadIcons();
    enumValues("species").then((s) => (species = s));
    invoke<Coded[]>("battle_coded_all", { owner: "ability" })
      .then((c) => (coded = c))
      .catch(() => {});
  });
  $effect(() => {
    void version;
    enumValues("ability").then((n) => (names = n));
    textBank(2, 183)
      .then((d) => (descriptions = d))
      .catch(() => {});
  });
  interface Check {
    place: string;
    what: string;
    function: string;
  }
  let checks = $state<Check[]>([]);
  $effect(() => {
    const at = current;
    checks = [];
    invoke<Check[]>("ability_checks", { id: at })
      .then((c) => {
        if (current === at) checks = c;
      })
      .catch(() => {});
  });

  $effect(() => {
    void version;
    const at = current;
    users = null;
    invoke<Users>("ability_users", { id: at })
      .then((u) => {
        if (current === at) users = u;
      })
      .catch(() => {});
  });

  const query = debounced(() => filter);
  const entries = $derived(names.map((name, index) => ({ index, name })).filter((e) => e.index > 0));
  const shown = $derived.by(() => {
    const q = query.value.trim().toLowerCase();
    return q ? entries.filter((e) => e.name.toLowerCase().includes(q) || String(e.index) === q) : entries;
  });
  const code = $derived(coded.find((c) => c.id === current) ?? null);
  const describe = (s: string | undefined) => (s ?? "").replace(/\\n/g, " ");

  function pick(index: number) {
    if (index === current) return;
    play("select");
    current = index;
  }
  function openOther(index: number) {
    pick(index);
    list?.reveal(shown.findIndex((e) => e.index === index));
  }
</script>

{#snippet group(label: string, list: number[])}
  {#if list.length}
    <h4>{label} ({list.length})</h4>
    <div class="chips">
      {#each list as sp}
        <span class="chip">{#if speciesIcon(sp)}<img class="px" src={speciesIcon(sp)} alt="" />{/if}{species[sp] ?? sp}</span>
      {/each}
    </div>
  {/if}
{/snippet}

<div class="tables">
  <aside class="entries">
    <input class="search" placeholder="Search {entries.length} abilities" aria-label="Search abilities" bind:value={filter} onkeydown={(e) => e.key === "Escape" && (filter = "")} />
    <VirtualList bind:this={list} items={shown} height={34} key={(e) => e.index} label="Abilities">
      {#snippet row(e)}
        <button class="row-item" aria-current={e.index === current} onclick={() => pick(e.index)}>
          <span class="num muted">{e.index}</span>
          <span class="label">{e.name}</span>
          {#if isAdded(e.index)}
            <span class="tag new" title="Added by this mod">new</span>
          {:else if coded.length && !coded.some((c) => c.id === e.index)}
            <span class="tag" title="No battle code of its own: the game tests its id directly">built in</span>
          {/if}
        </button>
      {/snippet}
      {#snippet empty()}
        <p class="muted pad">{names.length ? "Nothing matches." : "Loading..."}</p>
      {/snippet}
    </VirtualList>
  </aside>

  <section class="record">
    <div class="head">
      <div class="title">
        <h2>{names[current] ?? ""}</h2>
        <p class="muted">{describe(descriptions[current])}</p>
      </div>
      {#if !readOnly && added}
        <div class="acts">
          {#if isAdded(current)}
            <button onclick={() => ((dialog = "remove"), play("decide"))}>Remove ability</button>
          {/if}
          <button onclick={() => ((dialog = "add"), play("decide"))} title="A new ability made from a copy of this one">New ability</button>
        </div>
      {/if}
    </div>
    <div class="scroll body">
      {#if current > 0}
        <TextPanel kind="abilities" id={current} {readOnly} onSaved={() => (onSaved(), reloadNames())} />
      {/if}
      <div class="window block">
        <h3>What it does</h3>
        {#if checks.length}
          {#each ["battle", "outside battle"] as place}
            {#if checks.some((c) => c.place === place)}
              <h4>{place === "battle" ? "In battle" : "Outside battle"}</h4>
              <ul>
                {#each checks.filter((c) => c.place === place) as c}
                  <li>{c.what} <span class="muted">({c.function})</span></li>
                {/each}
              </ul>
            {/if}
          {/each}
        {/if}
        {#if code}
          <p>
            Battle code: <strong>{code.label ?? code.function}</strong>{#if code.function}<span class="muted"> ({code.function})</span>{/if}.
          </p>
          {#if code.sharedWith.length}
            <p>
              The same code runs for
              {#each code.sharedWith as other, i}{i ? ", " : ""}<button class="link" onclick={() => openOther(other)}>{names[other] ?? other}</button>{/each}.
            </p>
          {/if}
        {:else if checks.length}
          <p class="muted note">
            It has no battle code of its own: the game tests its id in the places above, so another ability cannot copy it.
          </p>
        {:else if isAdded(current)}
          <p class="note">Added by this mod. Pick what it works like under "In battle".</p>
        {:else}
          <p class="muted">No effect: nothing in the game's code uses this ability.</p>
        {/if}
      </div>
      {#if current > 0 && added}
        <BattleHandler table="abilities" id={current} last={added.last} {version} {readOnly} {onSaved} />
      {/if}
      <div class="window block">
        <h3>Pokemon with it</h3>
        {#if !users}
          <p class="muted">Looking...</p>
        {:else if !users.first.length && !users.second.length && !users.hidden.length}
          <p class="muted">No Pokemon has it.</p>
        {:else}
          {@render group("First ability", users.first)}
          {@render group("Second ability", users.second)}
          {@render group("Hidden ability", users.hidden)}
        {/if}
      </div>
    </div>
  </section>
</div>

{#if dialog}
  <ContentDialog
    mode={dialog}
    noun="ability"
    {names}
    from={current}
    removing={{ id: current, name: names[current] ?? "" }}
    onAdd={addAbility}
    onRemove={removeAbility}
    onClose={() => (dialog = null)}
  />
{/if}

<style>
  .tables {
    display: grid;
    grid-template-columns: 17rem minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    height: 100%;
  }
  .entries {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 2px solid var(--outline);
  }
  .search {
    margin: 0.5rem;
  }
  .row-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    box-shadow: none;
    border-radius: 0;
    font-weight: 500;
  }
  .tag.new {
    color: var(--accent-ink);
    background: var(--accent);
    border-color: var(--accent-dark);
  }
  .row-item[aria-current="true"] {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .row-item .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num {
    width: 2.2rem;
    text-align: right;
    font-size: 0.8rem;
  }
  .record {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .head {
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--outline);
    background: var(--bg);
    display: flex;
    align-items: flex-start;
    gap: 1rem;
  }
  .head .title {
    flex: 1;
    min-width: 0;
  }
  .acts {
    display: flex;
    gap: 0.5rem;
  }
  .body {
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    overflow-y: auto;
    min-height: 0;
  }
  .block {
    padding: 0.9rem 1rem;
  }
  .block p {
    margin: 0.35rem 0;
  }
  .note {
    font-size: 0.9rem;
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
  .link {
    background: none;
    border: none;
    box-shadow: none;
    padding: 0;
    color: var(--accent);
    font-weight: 600;
    cursor: pointer;
  }
</style>
