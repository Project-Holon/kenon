<script lang="ts">
  // Adds a move or an item made from an existing one, or removes one the
  // mod added (core/src/content.rs). Removing moves the later added ones
  // down one number; everything in the mod that uses them follows.
  import { onMount, untrack } from "svelte";
  import SearchSelect from "morphe/SearchSelect.svelte";
  import { play } from "./sfx";

  let {
    mode,
    noun,
    names,
    from = 1,
    removing = null,
    icon,
    onAdd,
    onRemove,
    onClose,
  }: {
    mode: "add" | "remove";
    /// "move" or "item".
    noun: string;
    /// Every entry's name, by id.
    names: string[];
    from?: number;
    /// The id and name of the entry to remove.
    removing?: { id: number; name: string } | null;
    icon?: (v: number) => string | null;
    onAdd: (from: number, name: string) => Promise<void>;
    onRemove: (id: number) => Promise<void>;
    onClose: () => void;
  } = $props();

  let dialog = $state<HTMLDialogElement | null>(null);
  let nameInput = $state<HTMLInputElement | null>(null);
  // The dialog starts from the entry open when it was opened.
  let source = $state(untrack(() => from));
  let name = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  onMount(() => {
    dialog?.showModal();
    nameInput?.focus();
  });

  async function add() {
    if (!name.trim()) {
      nameInput?.focus();
      return;
    }
    busy = true;
    error = null;
    try {
      await onAdd(source, name.trim());
      dialog?.close();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!removing) return;
    busy = true;
    error = null;
    try {
      await onRemove(removing.id);
      dialog?.close();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialog}
  class="content window"
  onclose={onClose}
  onclick={(e) => {
    if (e.target === dialog) dialog?.close();
  }}
  aria-label={mode === "add" ? `New ${noun}` : `Remove ${noun}`}
>
  {#if mode === "add"}
    <header>
      <h2>New {noun}</h2>
      <p class="muted">It starts as a copy of another {noun}. Change anything after.</p>
    </header>
    <div class="field">
      <span>Copy of</span>
      <SearchSelect bind:value={source} options={names} {icon} label="Copy of" />
    </div>
    <label class="field">
      <span>Name</span>
      <input
        bind:this={nameInput}
        bind:value={name}
        maxlength={noun === "Pokemon" ? 10 : 20}
        placeholder={({ move: "Storm Bolt", ability: "Super Intimidate", item: "MegaHyper Potion", Pokemon: "Voltwing" } as Record<string, string>)[noun] ?? ""}
        onkeydown={(e) => e.key === "Enter" && add()}
      />
    </label>
    {#if error}<p class="bad">{error}</p>{/if}
    <footer>
      <button class="quiet" onclick={() => dialog?.close()}>Cancel</button>
      <button class="primary" disabled={busy || !name.trim()} aria-busy={busy} onclick={add}>
        {#if busy}<span class="spin" aria-hidden="true"></span>Adding...{:else}Add {noun}{/if}
      </button>
    </footer>
  {:else if removing}
    <header>
      <h2>Remove {removing.name}?</h2>
      <p class="muted">
        The {noun === "ability" ? "abilities" : noun === "Pokemon" ? "Pokemon" : `${noun}s`} added after it shift down by one ID, and all references in this mod update.
        References to {removing.name} are cleared.
      </p>
    </header>
    {#if error}<p class="bad">{error}</p>{/if}
    <footer>
      <button class="quiet" onclick={() => dialog?.close()}>Cancel</button>
      <button class="primary" disabled={busy} aria-busy={busy} onclick={remove}>
        {#if busy}<span class="spin" aria-hidden="true"></span>Removing...{:else}Remove{/if}
      </button>
    </footer>
  {/if}
</dialog>

<style>
  .content {
    width: min(32rem, calc(100vw - 2rem));
    padding: 1.25rem 1.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    color: inherit;
    overflow: visible;
  }
  .content:not([open]) {
    display: none;
  }
  .content::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  header p {
    margin: 0.2rem 0 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    min-width: 0;
  }
  .field > span {
    font-size: 0.85rem;
    color: var(--muted);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
  .bad {
    color: var(--bad);
  }
  footer button {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
  }
  .spin {
    width: 0.9em;
    height: 0.9em;
    border: 2px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation-duration: 2s;
    }
  }
</style>
