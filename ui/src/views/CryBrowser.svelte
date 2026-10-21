<script lang="ts">
  import { onMount } from "svelte";
  import { asset } from "../lib/backend";
  import { tableEntries, type Entry } from "../lib/tables";
  import { loadIcons, speciesIcon } from "../lib/icons.svelte";
  import VirtualList from "../lib/VirtualList.svelte";
  import Thumb from "../lib/Thumb.svelte";
  import { debounced } from "../lib/cache.svelte";

  // Cries are one wave each, wave archive n for species n (names from the
  // ROM's species table).
  let list = $state<Entry[]>([]);
  let loaded = $state(false);
  let filter = $state("");
  let playing = $state<number | null>(null);
  let error = $state<string | null>(null);
  let audio: HTMLAudioElement | null = null;

  onMount(() => {
    loadIcons();
    tableEntries("species")
      .then((all) => (list = all.filter((e) => e.index >= 1 && e.index <= 649)))
      .catch((e) => (error = String(e)))
      .finally(() => (loaded = true));
    return () => audio?.pause();
  });

  const query = debounced(() => filter);
  const shown = $derived.by(() => {
    const q = query.value.trim().toLowerCase();
    return q ? list.filter((e) => e.name.toLowerCase().includes(q) || String(e.index) === q) : list;
  });

  function play(e: Entry) {
    audio?.pause();
    // Cleared first, so the interrupted play() below reads as stopped.
    audio = null;
    if (playing === e.index) {
      playing = null;
      return;
    }
    error = null;
    const a = new Audio(asset(`cry/${e.index}`));
    audio = a;
    // A cry that was stopped must not touch the state of the next one.
    a.onended = () => audio === a && (playing = null);
    const fail = () => {
      if (audio !== a) return;
      playing = null;
      error = `The cry of ${e.name} could not be played.`;
    };
    a.onerror = fail;
    playing = e.index;
    a.play().catch(fail);
  }
</script>

<div class="cries">
  <div class="head">
    <input class="search" placeholder={loaded ? `Search ${list.length} cries` : "Search"} bind:value={filter} />
  </div>
  {#if error}<p class="bad pad">{error}</p>{/if}
  <div class="scroll">
    <VirtualList items={shown} height={42} key={(e) => e.index} label="Cries">
      {#snippet row(e)}
        <div class="row-item" aria-current={playing === e.index}>
          <button class="play" onclick={() => play(e)}>{playing === e.index ? "Stop" : "Play"}</button>
          <span class="icon"><Thumb src={speciesIcon(e.index) ?? ""} /></span>
          <span class="label">{e.name}</span>
          <span class="tag" title="Pokedex number">#{e.index}</span>
        </div>
      {/snippet}
      {#snippet empty()}
        <p class="muted pad">{!loaded ? "Loading..." : list.length ? "Nothing matches." : "Nothing here."}</p>
      {/snippet}
    </VirtualList>
  </div>
</div>

<style>
  .cries {
    display: flex;
    flex-direction: column;
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
    flex: 1;
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
  .pad {
    padding: 1rem;
  }
</style>
