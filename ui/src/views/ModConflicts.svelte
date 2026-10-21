<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    modConflicts,
    modGetPaths,
    modSetPaths,
    type ConflictReport,
    type ModPaths,
  } from "../lib/modApi";
  import { play } from "../lib/sfx";

  let { readOnly }: { readOnly: boolean } = $props();

  let paths = $state<ModPaths>({ modsFolder: null, declaredSet: null, declaredFound: null });
  let report = $state<ConflictReport | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function scan() {
    report = null;
    error = null;
    if (!paths.modsFolder) return;
    busy = true;
    try {
      report = await modConflicts();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  onMount(async () => {
    if (readOnly) return;
    try {
      paths = await modGetPaths();
      await scan();
    } catch (e) {
      error = String(e);
    }
  });

  async function choose() {
    const picked = await open({ directory: true, multiple: false, title: "Choose the folder eidos loads mods from" });
    if (typeof picked !== "string") return;
    try {
      paths = await modSetPaths(picked, paths.declaredSet);
      play("decide");
      await scan();
    } catch (e) {
      error = String(e);
    }
  }

  async function clear() {
    paths = await modSetPaths(null, paths.declaredSet);
    report = null;
    error = null;
  }

  const others = $derived(report ? report.mods.filter((m) => !m.this).length : 0);
</script>

<div class="conflicts">
  <header>
    <h2>Conflicts</h2>
    <div class="spacer"></div>
    {#if !readOnly}
      {#if paths.modsFolder}<button class="quiet" onclick={clear}>Clear folder</button>{/if}
      <button onclick={choose}>Mods folder</button>
      <button disabled={!paths.modsFolder || busy} onclick={scan}>Check again</button>
    {/if}
  </header>

  {#if readOnly}
    <p class="muted pad">Open a project to compare it with other mods.</p>
  {:else}
    <div class="scroll">
      <p class="muted">
        {#if paths.modsFolder}
          Comparing with the mods in <code>{paths.modsFolder}</code>.
        {:else}
          Choose the folder eidos loads mods from. This mod is compared with each mod in it.
        {/if}
      </p>
      {#if error}<p class="bad">{error}</p>{/if}

      {#if report}
        <h3>Load order</h3>
        <ol class="order">
          {#each report.mods as m}
            <li class:this={m.this}>
              <span>{m.name}</span>
              <span class="muted">order {m.order}{m.this ? ", this mod" : ""}</span>
            </li>
          {/each}
        </ol>
        <p class="muted small">Mods with the same order are listed by folder name here. eidos keeps them in the order they were added.</p>

        <h3>Changed by this mod and another</h3>
        {#if report.conflicts.length}
          <table>
            <thead>
              <tr><th>Item</th><th>Mods, first to last</th><th>Wins</th></tr>
            </thead>
            <tbody>
              {#each report.conflicts as c}
                <tr>
                  <td>
                    <code>{c.key}</code>
                    <span class="tag">{c.kind === "hook" ? "hook" : "file"}</span>
                  </td>
                  <td>
                    {#each c.mods as m, i}
                      <span class:this={m.this}>{m.name}{c.detail[i] ? ` (${c.detail[i]})` : ""}</span>{i < c.mods.length - 1
                        ? ", "
                        : ""}
                    {/each}
                  </td>
                  <td>{c.winner}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <p class="ok">{others ? "No file or hook is changed by this mod and another." : "There are no other mods in the folder."}</p>
        {/if}
      {:else if busy}
        <p class="muted">Checking...</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .conflicts {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    height: 100%;
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .spacer {
    flex: 1;
  }
  .scroll {
    overflow-y: auto;
    padding: 1rem;
    display: grid;
    gap: 0.6rem;
    align-content: start;
  }
  .pad {
    padding: 1rem;
  }
  .order {
    margin: 0;
    padding-left: 1.4rem;
    display: grid;
    gap: 0.15rem;
  }
  .order li span:first-child {
    margin-right: 0.6rem;
  }
  .this {
    font-weight: 700;
  }
  .small {
    font-size: 0.85rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  th,
  td {
    text-align: left;
    padding: 0.35rem 0.5rem;
    border-bottom: 1px solid var(--rim);
    vertical-align: top;
    overflow-wrap: anywhere;
  }
  th:first-child {
    width: 40%;
  }
  th:last-child {
    width: 20%;
  }
</style>
