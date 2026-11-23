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
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialog}
  class="patch window"
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
          {#each report.code as line (line)}<li><code>{line}</code></li>{/each}
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
