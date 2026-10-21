<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { play } from "./sfx";

  // Everything the mod changes, with the game's value and the mod's, and
  // a way to undo each change or all of them.
  let { onClose, onChanged }: { onClose: () => void; onChanged: () => void } = $props();

  interface DiffLine {
    label: string;
    old: string;
    new: string;
  }
  interface ChangeInfo {
    path: string;
    member: [number, number] | null;
    category: string;
    title: string;
    lines: DiffLine[];
    more: number;
  }

  let list = $state<ChangeInfo[] | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let open = $state<Record<string, boolean>>({});
  let confirmAll = $state(false);
  let dialog = $state<HTMLDialogElement | null>(null);

  async function load() {
    try {
      list = await invoke<ChangeInfo[]>("describe_changes");
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    dialog?.showModal();
    load();
  });

  async function undo(paths: string[]) {
    busy = true;
    error = null;
    try {
      for (const path of paths) await invoke("revert_change", { path });
      play("cancel");
      onChanged();
      confirmAll = false;
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  const groups = $derived.by(() => {
    const out = new Map<string, ChangeInfo[]>();
    for (const c of list ?? []) out.set(c.category, [...(out.get(c.category) ?? []), c]);
    return [...out];
  });
</script>

<dialog bind:this={dialog} class="changes window" onclose={onClose} aria-label="Changes">
  <header>
    <h2>Changes</h2>
    <span class="muted">{list ? `${list.length} file${list.length === 1 ? "" : "s"}` : ""}</span>
    <div class="spacer"></div>
    {#if list?.length}
      {#if confirmAll}
        <span>Undo every change?</span>
        <button class="danger" disabled={busy} onclick={() => undo(list!.map((c) => c.path))}>Undo all</button>
        <button onclick={() => (confirmAll = false)}>Keep</button>
      {:else}
        <button onclick={() => (confirmAll = true)}>Undo all</button>
      {/if}
    {/if}
    <button onclick={() => dialog?.close()} aria-label="Close">Close</button>
  </header>

  {#if error}<p class="bad">{error}</p>{/if}

  <div class="body">
    {#if list === null}
      <p class="muted">Loading...</p>
    {:else if !list.length}
      <p class="muted">No changes.</p>
    {:else}
      {#each groups as [category, items]}
        <section>
          <h3>{category} <span class="muted">{items.length}</span></h3>
          {#each items as c (c.path)}
            <article class="change">
              <div class="row">
                <button class="title quiet" onclick={() => (open[c.path] = !open[c.path])} aria-expanded={open[c.path] ?? true}>
                  <span class="caret">{(open[c.path] ?? true) ? "v" : ">"}</span>
                  <strong>{c.title}</strong>
                  <span class="muted path">{c.path}</span>
                </button>
                <button disabled={busy} onclick={() => undo([c.path])}>Undo</button>
              </div>
              {#if (open[c.path] ?? true) && c.lines.length}
                <table>
                  <tbody>
                    {#each c.lines as l}
                      <tr>
                        <th>{l.label}</th>
                        <td class="old">{l.old}</td>
                        <td class="arrow">{l.old ? "->" : ""}</td>
                        <td class="new">{l.new}</td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
                {#if c.more}<p class="muted small">{c.more} more</p>{/if}
              {/if}
            </article>
          {/each}
        </section>
      {/each}
    {/if}
  </div>
</dialog>

<style>
  .changes {
    width: min(56rem, calc(100vw - 2rem));
    max-height: calc(100vh - 4rem);
    padding: 0;
    display: flex;
    flex-direction: column;
    color: inherit;
  }
  .changes:not([open]) {
    display: none;
  }
  .changes::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.6rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  header h2 {
    margin: 0;
    font-size: 1.1rem;
  }
  header button {
    padding: 0.2rem 0.7rem;
  }
  .spacer {
    flex: 1;
  }
  .body {
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0.75rem 1rem 1rem;
    min-height: 0;
  }
  section h3 {
    margin: 0.8rem 0 0.4rem;
    font-size: 0.95rem;
  }
  .change {
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    margin-bottom: 0.5rem;
    overflow: hidden;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3rem 0.5rem;
  }
  .row button:last-child {
    padding: 0.15rem 0.7rem;
  }
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    text-align: left;
    box-shadow: none;
  }
  .title strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .caret {
    font-family: monospace;
    opacity: 0.6;
  }
  .path {
    font-family: monospace;
    font-size: 0.75rem;
    white-space: nowrap;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
    font-size: 0.85rem;
  }
  tr {
    border-top: 1px solid color-mix(in srgb, var(--outline) 40%, transparent);
  }
  th {
    width: 30%;
    text-align: left;
    font-weight: 600;
    padding: 0.25rem 0.6rem;
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  td {
    padding: 0.25rem 0.4rem;
    overflow-wrap: anywhere;
    font-family: monospace;
  }
  .old {
    background: color-mix(in srgb, #e5484d 16%, transparent);
    text-decoration: line-through;
    text-decoration-color: color-mix(in srgb, #e5484d 60%, transparent);
  }
  .new {
    background: color-mix(in srgb, #30a46c 18%, transparent);
  }
  .arrow {
    width: 2rem;
    text-align: center;
    opacity: 0.6;
  }
  .small {
    font-size: 0.8rem;
    padding: 0 0.6rem 0.4rem;
  }
  .danger {
    background: #e5484d;
    color: #fff;
  }
</style>
