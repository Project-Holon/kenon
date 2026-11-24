<script lang="ts">
  // What a ROM hack patch became: the changes the project now holds, and
  // what a mod cannot carry. Opening the project is the next step.
  import { onMount } from "svelte";
  import type { PatchImport } from "./backend";
  import { play } from "./sfx";

  let {
    report,
    onOpen,
    onClose,
  }: {
    report: PatchImport;
    onOpen: (root: string) => void;
    onClose: () => void;
  } = $props();

  let dialog = $state<HTMLDialogElement | null>(null);
  onMount(() => dialog?.showModal());

  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;
  const notCarried = $derived(report.code.length + report.leftOut.length);
  // Code parts shown side by side, by summary.
  let shown = $state<Record<string, boolean>>({});
  const wide = $derived(Object.values(shown).some((v) => v));
  const hex = (n: number) => n.toString(16).padStart(8, "0");
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialog}
  class="patch window"
  class:wide
  onclose={onClose}
  onclick={(e) => {
    if (e.target === dialog) dialog?.close();
  }}
  aria-label="Patch imported"
>
  <header>
    <h2>{report.name}</h2>
    <p class="muted">
      {plural(report.members, "change")} in {plural(report.archives, "archive")}{report.files.length
        ? `, ${plural(report.files.length, "file")}`
        : ""}. You can edit them all like your own changes.
    </p>
  </header>

  {#if notCarried > 0}
    <section>
      <h3>Not in the mod</h3>
      {#if report.code.length}
        <p class="muted small">eidos runs its own build of the game's code, so code a patch changes is not carried.</p>
        <ul>
          {#each report.code as change (change.summary)}
            <li>
              <code>{change.summary}</code>
              {#if change.views.length}
                <button class="quiet show" onclick={() => (shown[change.summary] = !shown[change.summary])}>
                  {shown[change.summary] ? "Hide the code" : "Show the code"}
                </button>
              {/if}
              {#if shown[change.summary]}
                {#each change.views as view (view.start)}
                  <div class="view">
                    {#if view.functions.length}
                      <p class="small">
                        Changes {#each view.functions as f, i}{i ? ", " : ""}<b>{f.name}</b>
                          <span class="muted">({f.thumb ? "Thumb" : "ARM"}, src/game/{f.file} in aletheia)</span>{/each}
                      </p>
                    {/if}
                    {#if view.note}<p class="muted small">{view.note}</p>{/if}
                    {#each [{ label: "Game", lines: view.game, kind: "old" }, { label: "Patch", lines: view.patch, kind: "new" }] as side (side.label)}
                      <div class="side">
                        <span class="label">{side.label}</span>
                        <table>
                          <tbody>
                            {#each side.lines as l (l.address)}
                              <tr class={l.changed ? side.kind : ""}>
                                <td class="addr">{hex(l.address)}</td>
                                <td class="bytes">{l.bytes}</td>
                                <td class="text">{l.text}</td>
                                <td class="note">{l.note}</td>
                              </tr>
                            {/each}
                          </tbody>
                        </table>
                      </div>
                    {/each}
                  </div>
                {/each}
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
      {#if report.leftOut.length}
        <ul>
          {#each report.leftOut as line (line)}<li>{line}</li>{/each}
        </ul>
      {/if}
    </section>
  {/if}

  <footer>
    <button onclick={() => (play("cancel"), dialog?.close())}>Close</button>
    <button class="primary" onclick={() => (dialog?.close(), onOpen(report.root))}>Open project</button>
  </footer>
</dialog>

<style>
  .patch {
    width: min(38rem, calc(100vw - 2rem));
    max-height: calc(100vh - 4rem);
    padding: 1.25rem 1.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    color: inherit;
    overflow-y: auto;
  }
  .patch.wide {
    width: min(64rem, calc(100vw - 2rem));
  }
  .show {
    margin-left: 0.5rem;
    padding: 0.1rem 0.6rem;
    font-size: 0.8rem;
  }
  .view {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    margin: 0.5rem 0 0.75rem;
  }
  .side {
    border: 1px solid var(--rim);
    border-radius: var(--radius-s);
    overflow-x: auto;
  }
  .side .label {
    display: block;
    padding: 0.2rem 0.5rem;
    font-size: 0.8rem;
    color: var(--muted);
    border-bottom: 1px solid var(--rim);
  }
  table {
    border-collapse: collapse;
    width: 100%;
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 0.78rem;
  }
  td {
    padding: 0.05rem 0.5rem;
    white-space: nowrap;
  }
  td.note {
    white-space: normal;
    color: var(--muted);
    width: 100%;
  }
  td.addr,
  td.bytes {
    color: var(--muted);
  }
  tr.old {
    background: color-mix(in srgb, #e5484d 16%, transparent);
  }
  tr.new {
    background: color-mix(in srgb, #30a46c 18%, transparent);
  }
  .patch:not([open]) {
    display: none;
  }
  .patch::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  header h2,
  h3 {
    margin: 0;
  }
  header p {
    margin: 0.3rem 0 0;
  }
  h3 {
    font-size: 0.95rem;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.6rem 0.75rem;
    border-radius: var(--radius-s);
    background: var(--panel-2);
    border: 1px solid var(--rim);
  }
  ul {
    margin: 0;
    padding-left: 1.1rem;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  code {
    font-size: 0.8rem;
    overflow-wrap: anywhere;
  }
  .small {
    font-size: 0.85rem;
    margin: 0;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
