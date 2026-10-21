<script lang="ts">
  import { onMount } from "svelte";
  import { asset } from "../lib/backend";
  import { tableEntries, type Entry } from "../lib/tables";
  import VirtualList from "../lib/VirtualList.svelte";
  import Thumb from "../lib/Thumb.svelte";
  import { debounced } from "../lib/cache.svelte";

  // Cries are one wave each, wave archive n for species n (names from the
  // ROM's species table).
  let list = $state<Entry[]>([]);
  let filter = $state("");
  let playing = $state<number | null>(null);
  let error = $state<string | null>(null);
  let audio: HTMLAudioElement | null = null;

  onMount(() => {
    tableEntries("species")
      .then((all) => (list = all.filter((e) => e.index >= 1 && e.index <= 649)))
      .catch((e) => (error = String(e)));
    return () => audio?.pause();
  });

  const query = debounced(() => filter);
  const shown = $derived.by(() => {
    const q = query.value.trim().toLowerCase();
    return q ? list.filter((e) => e.name.toLowerCase().includes(q) || String(e.index) === q) : list;
  });

  function play(e: Entry) {
    audio?.pause();
    if (playing === e.index) {
      playing = null;
      return;
    }
    audio = new Audio(asset(`cry/${e.index}`));
    audio.onended = () => (playing = null);
    audio.onerror = () => {
      playing = null;
      error = `The cry of ${e.name} could not be played.`;
    };
    playing = e.index;
    audio.play().catch(() => {});
  }
</script>

<div class="cries">
  <div class="head">
    <input class="search" placeholder="Search {list.length} cries" bind:value={filter} />
    <p class="muted small">Each cry is the Pokemon's own recording in the game's sound archive.</p>
  </div>
  {#if error}<p class="bad pad">{error}</p>{/if}
  <div class="scroll">
    <VirtualList items={shown} height={42} key={(e) => e.index} label="Cries">
      {#snippet row(e)}
        <div class="row-item" aria-current={playing === e.index}>
          <button class="play" onclick={() => play(e)}>{playing === e.index ? "Stop" : "Play"}</button>
          <span class="icon"><Thumb src={asset(`preview/7:${7 + 2 * e.index}/7:0?cells=2`)} /></span>
          <span class="label">{e.name}</span>
          <span class="tag">#{e.index}</span>
        </div>
      {/snippet}
      {#snippet empty()}
        <p class="muted pad">{list.length ? "Nothing matches." : "Loading..."}</p>
      {/snippet}
    </VirtualList>
  </div>
</div>

<style>
  .cries {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr);
    height: 100%;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .search {
    width: min(22rem, 100%);
  }
  .scroll {
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 0.5rem 1rem;
  }
  .play {
    min-width: 4.2rem;
    padding: 0.2rem 0.6rem;
  }
  .icon {
    width: 32px;
    height: 32px;
    overflow: hidden;
    flex: none;
  }
  .icon :global(img) {
    height: 32px;
  }
  .small {
    font-size: 0.85rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
