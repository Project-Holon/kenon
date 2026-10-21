<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import { debounced } from "../lib/cache.svelte";
  import { gameText, members, saveTextBank, textBank, type MemberInfo } from "../lib/backend";
  import { play } from "../lib/sfx";

  let {
    archive,
    readOnly,
    version,
    onSaved,
  }: { archive: number; readOnly: boolean; version: number; onSaved: () => void } = $props();

  let banks = $state<MemberInfo[]>([]);
  let bank = $state<number | null>(null);
  let strings = $state<string[]>([]);
  let original = $state<string[]>([]);
  let focused = $state<number | null>(null);
  let filter = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);
  let loading = $state(false);
  // Unsaved edits of banks left behind, so switching banks loses nothing.
  const drafts = new SvelteMap<number, string[]>();
  let seq = 0;
  // Strings are drawn in pages of 80 as the list scrolls.
  let limit = $state(80);
  let sentinel = $state<HTMLDivElement | null>(null);

  $effect(() => {
    if (!sentinel) return;
    const observer = new IntersectionObserver((seen) => {
      if (seen.some((e) => e.isIntersecting)) limit += 80;
    });
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  onMount(async () => {
    try {
      banks = await members(archive);
      if (banks.length) await open(banks[0].index, false);
    } catch (e) {
      error = String(e);
    }
  });

  async function open(index: number, sound = true) {
    if (index === bank) return;
    if (sound) play("select");
    if (bank !== null && dirty) drafts.set(bank, strings.slice());
    else if (bank !== null) drafts.delete(bank);
    error = null;
    bank = index;
    focused = null;
    limit = 80;
    loading = true;
    const my = ++seq;
    try {
      const fresh = await textBank(archive, index);
      if (my !== seq) return;
      original = fresh;
      strings = drafts.get(index) ?? fresh.slice();
      drafts.delete(index);
    } catch (e) {
      if (my === seq) {
        error = String(e);
        strings = [];
        original = [];
      }
    } finally {
      if (my === seq) loading = false;
    }
  }

  // Up and down in the bank list step through the banks.
  function step(e: KeyboardEvent) {
    const d = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (!d || !banks.length) return;
    e.preventDefault();
    const at = banks.findIndex((b) => b.index === bank);
    const to = banks[Math.max(0, Math.min(banks.length - 1, at + d))];
    open(to.index);
    document.querySelector<HTMLElement>(`[data-bank="${to.index}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function keys(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      if (!readOnly && dirty && !busy) save();
    }
  }

  const dirty = $derived(strings.some((s, i) => s !== original[i]));
  const query = debounced(() => filter);
  // Show a number to jump to that string; any other text searches the strings.
  const shown = $derived.by(() => {
    const q = query.value.trim().toLowerCase();
    const all = strings.map((text, i) => ({ text, i }));
    return q ? all.filter(({ text, i }) => String(i) === q || text.toLowerCase().includes(q)) : all;
  });
  $effect(() => {
    void query.value;
    limit = 80;
  });

  async function save() {
    if (bank === null) return;
    busy = true;
    error = null;
    try {
      await saveTextBank(archive, bank, strings);
      original = strings.slice();
      drafts.delete(bank);
      play("save");
      banks = await members(archive);
      onSaved();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  // Control codes ({...}) and escapes stay as written in the preview.
  const preview = (s: string) => s.replaceAll("\\n", "\n");
</script>

<div class="text" role="presentation" onkeydown={keys}>
  <aside class="banks" onkeydown={step} role="presentation">
    <h3 class="muted">Banks</h3>
    <div class="scroll">
      {#each banks as b (b.index)}
        <button class="row-item" data-bank={b.index} aria-current={b.index === bank} onclick={() => open(b.index)}>
          <span class="label">Bank {b.index}</span>
          {#if (b.index === bank ? dirty : drafts.has(b.index))}
            <span class="tag changed" title="Not saved yet">edited</span>
          {:else if b.changed}
            <span class="tag changed">changed</span>
          {/if}
        </button>
      {/each}
    </div>
  </aside>

  <section class="strings">
    <div class="head">
      <input class="search" placeholder="Search this bank" aria-label="Search this bank" bind:value={filter} onkeydown={(e) => e.key === "Escape" && (filter = "")} />
      {#if strings.length}<span class="muted count">{shown.length === strings.length ? `${strings.length} strings` : `${shown.length} of ${strings.length}`}</span>{/if}
      <div class="spacer"></div>
      {#if readOnly}
        <span class="tag">read only</span>
      {:else}
        <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
      {/if}
    </div>
    {#if error}<p class="bad pad">{error}</p>{/if}
    <div class="scroll list">
      {#each shown.slice(0, limit) as { i } (i)}
        <div class="entry" class:changed={strings[i] !== original[i]}>
          <span class="num muted">{i}</span>
          <div class="fields">
            <textarea
              rows={Math.min(4, strings[i].split("\\n").length)}
              readonly={readOnly}
              aria-label="String {i}"
              bind:value={strings[i]}
              onfocus={() => (focused = i)}
            ></textarea>
            {#if strings[i] !== original[i] && !readOnly}
              <button class="quiet undo" onclick={() => (strings[i] = original[i] ?? "")}>Undo</button>
            {/if}
            {#if focused === i && strings[i]}
              <div class="window preview">
                <img class="px" src={gameText(preview(strings[i]))} alt="" />
              </div>
            {/if}
          </div>
        </div>
      {:else}
        <p class="muted pad">{loading || (!strings.length && !error) ? "Loading..." : strings.length ? "Nothing matches." : "This bank is empty."}</p>
      {/each}
      {#if shown.length > limit}<div bind:this={sentinel} class="more muted">Loading more...</div>{/if}
    </div>
    <p class="hint muted">
      <code>\n</code> is a new line; <code>&#123;...&#125;</code> is a game code (names, colours, pauses), kept
      as written.
    </p>
  </section>
</div>

<style>
  .text {
    display: grid;
    grid-template-columns: 11rem minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    height: 100%;
  }
  .banks {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 2px solid var(--outline);
    padding: 0.75rem 0.5rem;
  }
  .banks h3 {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin: 0 0.5rem 0.4rem;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .strings {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .head,
  .hint {
    flex: none;
  }
  .count {
    font-size: 0.85rem;
    white-space: nowrap;
  }
  .undo {
    align-self: flex-start;
    padding: 0.05rem 0.5rem;
    font-size: 0.8rem;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .search {
    width: min(22rem, 100%);
  }
  .spacer {
    flex: 1;
  }
  .list {
    padding: 0.5rem 1rem 1rem;
  }
  .entry {
    display: grid;
    grid-template-columns: 3rem minmax(0, 1fr);
    gap: 0.5rem;
    padding: 0.25rem 0;
  }
  .entry.changed textarea {
    border-color: var(--gold);
  }
  .num {
    text-align: right;
    padding-top: 0.4rem;
    font-variant-numeric: tabular-nums;
  }
  .fields {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    min-width: 0;
  }
  textarea {
    width: 100%;
    resize: vertical;
    field-sizing: content;
    min-height: 2.2rem;
    max-height: 10rem;
  }
  .preview {
    align-self: flex-start;
    max-width: 100%;
    overflow: hidden;
    background: #f8f8f8;
    padding: 0.5rem 0.75rem;
  }
  .preview img {
    zoom: 2;
    display: block;
    max-width: 100%;
  }
  .hint {
    padding: 0.5rem 1rem;
    font-size: 0.85rem;
    border-top: 2px solid var(--outline);
  }
  .pad {
    padding: 1rem;
  }
</style>
