<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { asset, audioChanges, importCry, revertCry, type AudioSummary } from "../lib/backend";
  import { tableEntries, type Entry } from "../lib/tables";
  import { loadIcons, speciesIcon } from "../lib/icons.svelte";
  import Thumb from "../lib/Thumb.svelte";
  import { debounced } from "morphe/cache.svelte";
  import { copyText, guarded, saveWav } from "../lib/exporter";
  import { openMenu, toast } from "morphe/menu.svelte";

  // Cries are one wave each, wave archive n for species n (names from the
  // ROM's species table).
  let list = $state<Entry[]>([]);
  let loaded = $state(false);
  let filter = $state("");
  let playing = $state<number | null>(null);
  let error = $state<string | null>(null);
  let audio: HTMLAudioElement | null = null;
  let selected = $state<Entry | null>(null);
  // Species whose cry the project replaced, and a counter that makes the
  // web view fetch a replaced cry again.
  let changed = $state<Set<number>>(new Set());
  let version = $state(0);
  let busy = $state(false);

  const refresh = () =>
    audioChanges()
      .then((c) => (changed = new Set(c.cries)))
      .catch(() => {});

  onMount(() => {
    loadIcons();
    refresh();
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

  const menu = (ev: MouseEvent, e: Entry) =>
    openMenu(ev, [
      { label: playing === e.index ? "Stop" : "Play", run: () => play(e) },
      { label: "Save as WAV", run: guarded(() => saveWav(`cry/${e.index}`, `${e.name} cry`)) },
      "-",
      { label: "Replace with WAV...", run: () => replace(e) },
      { label: "Revert to game", run: () => revert(e), disabled: !changed.has(e.index) },
      "-",
      { label: "Copy name", run: () => copyText(e.name) },
    ]);

  // "0.8 s, 10512 Hz, PCM8" and what the import did to the file.
  const describe = (s: AudioSummary) =>
    `${s.seconds.toFixed(1)} s, ${s.rate} Hz, ${s.format}.${s.note ? " " + s.note : ""}`;

  async function replace(e: Entry) {
    selected = e;
    const path = await open({ title: "Choose a WAV", filters: [{ name: "WAV sound", extensions: ["wav"] }] });
    if (typeof path !== "string") return;
    busy = true;
    error = null;
    try {
      const summary = await importCry(e.index, path);
      version++;
      await refresh();
      toast(`${e.name} cry replaced: ${describe(summary)}`);
      // Played at once, from the start.
      playing = null;
      play(e);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function revert(e: Entry) {
    error = null;
    try {
      await revertCry(e.index);
      version++;
      await refresh();
      toast(`${e.name} cry is the game's again`);
    } catch (err) {
      error = String(err);
    }
  }

  function play(e: Entry) {
    selected = e;
    audio?.pause();
    // Cleared first, so the interrupted play() below reads as stopped.
    audio = null;
    if (playing === e.index) {
      playing = null;
      return;
    }
    error = null;
    const a = new Audio(asset(`cry/${e.index}`, version));
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
    {#if selected}
      <span class="muted pick">{selected.name}</span>
      <button disabled={busy} onclick={() => replace(selected!)}>Replace with WAV...</button>
      {#if changed.has(selected.index)}<button onclick={() => revert(selected!)}>Revert to game</button>{/if}
    {/if}
  </div>
  {#if error}<p class="bad pad">{error}</p>{/if}
  <!-- A grid of cards: click one to play it (again to stop) and pick it for Replace. -->
  <div class="scroll">
    {#if !shown.length}
      <p class="muted pad">{!loaded ? "Loading..." : list.length ? "Nothing matches." : "Nothing here."}</p>
    {/if}
    <div class="grid" aria-label="Cries">
      {#each shown as e (e.index)}
        <button
          class="card"
          aria-pressed={playing === e.index}
          class:picked={selected?.index === e.index}
          class:playing={playing === e.index}
          onclick={() => ((selected = e), play(e))}
          oncontextmenu={(ev) => ((selected = e), menu(ev, e))}
          title={playing === e.index ? "Stop" : "Play"}
        >
          <span class="icon"><Thumb src={speciesIcon(e.index) ?? ""} /></span>
          <span class="label">{e.name}</span>
          {#if changed.has(e.index)}<span class="tag changed">Replaced</span>{/if}
          <span class="num muted">{e.index}</span>
        </button>
      {/each}
    </div>
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
  .pick {
    max-width: 12rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0.75rem 1rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
    gap: 0.4rem;
  }
  .card {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.25rem 0.5rem;
    text-align: left;
    font-weight: 600;
    min-width: 0;
  }
  .card .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card .num {
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
  }
  .card.playing {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .card.playing .num {
    color: inherit;
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
  .card.picked {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .pad {
    padding: 1rem;
  }
</style>
