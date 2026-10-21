<script lang="ts">
  import { onMount } from "svelte";
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

  onMount(async () => {
    try {
      banks = await members(archive);
      if (banks.length) await open(banks[0].index, false);
    } catch (e) {
      error = String(e);
    }
  });

  async function open(index: number, sound = true) {
    if (sound) play("select");
    error = null;
    bank = index;
    focused = null;
    strings = await textBank(archive, index);
    original = strings.slice();
  }

  const dirty = $derived(strings.some((s, i) => s !== original[i]));
  const shown = $derived(
    strings
      .map((text, i) => ({ text, i }))
      .filter(({ text }) => text.toLowerCase().includes(filter.trim().toLowerCase())),
  );

  async function save() {
    if (bank === null) return;
    busy = true;
    error = null;
    try {
      await saveTextBank(archive, bank, strings);
      original = strings.slice();
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

<div class="text">
  <aside class="banks">
    <h3 class="muted">Banks</h3>
    <div class="scroll">
      {#each banks as b (b.index)}
        <button class="row-item" aria-current={b.index === bank} onclick={() => open(b.index)}>
          <span class="label">Bank {b.index}</span>
          {#if b.changed}<span class="tag changed">changed</span>{/if}
        </button>
      {/each}
    </div>
  </aside>

  <section class="strings">
    <div class="head">
      <input class="search" placeholder="Search this bank" bind:value={filter} />
      <div class="spacer"></div>
      {#if readOnly}
        <span class="tag">read only</span>
      {:else}
        <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
      {/if}
    </div>
    {#if error}<p class="bad pad">{error}</p>{/if}
    <div class="scroll list">
      {#each shown as { i } (i)}
        <div class="entry" class:changed={strings[i] !== original[i]}>
          <span class="num muted">{i}</span>
          <div class="fields">
            <textarea
              rows={Math.min(4, strings[i].split("\\n").length)}
              readonly={readOnly}
              bind:value={strings[i]}
              onfocus={() => (focused = i)}
            ></textarea>
            {#if focused === i && strings[i]}
              <div class="window preview">
                <img class="px" src={gameText(preview(strings[i]))} alt="" />
              </div>
            {/if}
          </div>
        </div>
      {:else}
        <p class="muted pad">{strings.length ? "Nothing matches." : "Loading..."}</p>
      {/each}
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
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    min-width: 0;
    min-height: 0;
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
