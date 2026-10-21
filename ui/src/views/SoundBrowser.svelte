<script lang="ts">
  import { onMount } from "svelte";
  import { asset, sounds, type SoundInfo } from "../lib/backend";
  import VirtualList from "../lib/VirtualList.svelte";
  import { debounced } from "../lib/cache.svelte";

  let { group }: { group: SoundInfo["group"] } = $props();

  let list = $state<SoundInfo[]>([]);
  let filter = $state("");
  let playing = $state<number | null>(null);
  let loading = $state<number | null>(null);
  let error = $state<string | null>(null);
  let audio: HTMLAudioElement | null = null;

  onMount(() => {
    sounds()
      .then((all) => (list = all.filter((s) => s.group === group)))
      .catch((e) => (error = String(e)));
    return () => audio?.pause();
  });

  const query = debounced(() => filter);
  const shown = $derived.by(() => {
    const q = query.value.trim().toLowerCase();
    return q ? list.filter((s) => s.name.toLowerCase().includes(q) || String(s.index) === q) : list;
  });

  // "SEQ_BGM_E_ROUTE1" reads as "E route1".
  const pretty = (name: string) =>
    name
      .replace(/^SEQ_(BGM|SE|ME|PV)_?/, "")
      .replaceAll("_", " ")
      .toLowerCase()
      .replace(/^./, (c) => c.toUpperCase());

  function toggle(s: SoundInfo) {
    audio?.pause();
    if (playing === s.index) {
      playing = null;
      return;
    }
    loading = s.index;
    // Music loops: render a minute of it.
    audio = new Audio(asset(`sound/${s.index}?seconds=${group === "music" ? 60 : 10}`));
    audio.oncanplay = () => (loading = null);
    audio.onended = () => (playing = null);
    audio.onerror = () => {
      loading = null;
      playing = null;
      error = `${s.name} could not be played.`;
    };
    playing = s.index;
    audio.play().catch(() => {});
  }
</script>

<div class="sounds">
  <div class="head">
    <input class="search" placeholder="Search {list.length} sounds" bind:value={filter} />
    <p class="muted small">Rendered from the game's own sequences and instruments. Editing sound comes later.</p>
  </div>
  {#if error}<p class="bad pad">{error}</p>{/if}
  <div class="scroll">
    <VirtualList items={shown} height={42} key={(s) => s.index} label="Sounds">
      {#snippet row(s)}
        <div class="row-item sound" aria-current={playing === s.index}>
          <button class="play" onclick={() => toggle(s)} aria-label={playing === s.index ? "Stop" : "Play"}>
            {loading === s.index ? "..." : playing === s.index ? "Stop" : "Play"}
          </button>
          <span class="label">{pretty(s.name)}</span>
          <code class="muted name">{s.name}</code>
          <span class="tag">#{s.index}</span>
        </div>
      {/snippet}
      {#snippet empty()}
        <p class="muted pad">{list.length ? "Nothing matches." : "Loading..."}</p>
      {/snippet}
    </VirtualList>
  </div>
</div>

<style>
  .sounds {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
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
    padding: 0.5rem 1rem 0.5rem;
  }
  .sound .play {
    min-width: 4.2rem;
    padding: 0.2rem 0.6rem;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 40%;
  }
  .row-item[aria-current="true"] .muted {
    color: inherit;
  }
  .small {
    font-size: 0.85rem;
  }
  .pad {
    padding: 1rem;
  }
</style>
