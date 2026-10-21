<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { asset, audioChanges, importSound, revertSound, sounds, type AudioSummary, type SoundInfo } from "../lib/backend";
  import VirtualList from "../lib/VirtualList.svelte";
  import { debounced } from "../lib/cache.svelte";
  import { copyText, guarded, saveWav } from "../lib/exporter";
  import { openMenu, toast } from "../lib/menu.svelte";

  let { group }: { group: SoundInfo["group"] } = $props();

  let list = $state<SoundInfo[]>([]);
  let filter = $state("");
  let playing = $state<number | null>(null);
  let loading = $state<number | null>(null);
  let loaded = $state(false);
  let error = $state<string | null>(null);
  let audio: HTMLAudioElement | null = null;
  let selected = $state<SoundInfo | null>(null);
  // Sounds whose wave the project replaced, and a counter that makes the
  // web view fetch a replaced sound again.
  let changed = $state<Set<number>>(new Set());
  let version = $state(0);
  let busy = $state(false);

  const refresh = () =>
    audioChanges()
      .then((c) => (changed = new Set(c.sounds)))
      .catch(() => {});

  onMount(() => {
    refresh();
    sounds()
      .then((all) => (list = all.filter((s) => s.group === group)))
      .catch((e) => (error = String(e)))
      .finally(() => (loaded = true));
    return () => audio?.pause();
  });

  // What the list holds, for the search box.
  const noun = $derived(group === "music" ? "songs" : group === "jingle" ? "jingles" : "sound effects");

  const query = debounced(() => filter);
  const shown = $derived.by(() => {
    const q = query.value.trim().toLowerCase();
    return q
      ? list.filter((s) => s.name.toLowerCase().includes(q) || pretty(s.name).toLowerCase().includes(q) || String(s.index) === q)
      : list;
  });

  // "SEQ_BGM_E_ROUTE1" reads as "E route1".
  const pretty = (name: string) =>
    name
      .replace(/^SEQ_(BGM|SE|ME|PV)_?/, "")
      .replaceAll("_", " ")
      .toLowerCase()
      .replace(/^./, (c) => c.toUpperCase());

  // Saved songs run to their end, up to four minutes.
  const menu = (e: MouseEvent, s: SoundInfo) =>
    openMenu(e, [
      { label: playing === s.index ? "Stop" : "Play", run: () => toggle(s) },
      {
        label: "Save as WAV",
        run: guarded(() => saveWav(`sound/${s.index}?seconds=${group === "music" ? 240 : 30}`, pretty(s.name))),
      },
      "-",
      { label: "Replace with WAV...", run: () => replace(s) },
      { label: "Revert to game", run: () => revert(s), disabled: !changed.has(s.index) },
      "-",
      { label: "Copy name", run: () => copyText(pretty(s.name)) },
    ]);

  // "0.4 s, 22050 Hz, IMA-ADPCM" and what the import did to the file.
  const describe = (r: AudioSummary) =>
    `${r.seconds.toFixed(1)} s, ${r.rate} Hz, ${r.format}.${r.note ? " " + r.note : ""}`;

  async function replace(s: SoundInfo) {
    selected = s;
    const path = await open({ title: "Choose a WAV", filters: [{ name: "WAV sound", extensions: ["wav"] }] });
    if (typeof path !== "string") return;
    busy = true;
    error = null;
    try {
      const summary = await importSound(s.index, path);
      version++;
      await refresh();
      toast(`${pretty(s.name)} replaced: ${describe(summary)}`);
      // Played at once, from the start.
      playing = null;
      toggle(s);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function revert(s: SoundInfo) {
    error = null;
    try {
      await revertSound(s.index);
      version++;
      await refresh();
      toast(`${pretty(s.name)} is the game's again`);
    } catch (err) {
      error = String(err);
    }
  }

  function toggle(s: SoundInfo) {
    selected = s;
    audio?.pause();
    // Cleared first, so the interrupted play() below reads as stopped.
    audio = null;
    loading = null;
    if (playing === s.index) {
      playing = null;
      return;
    }
    error = null;
    loading = s.index;
    // Music loops: render a minute of it.
    const a = new Audio(asset(`sound/${s.index}?seconds=${group === "music" ? 60 : 10}`, version));
    audio = a;
    // A clip that was stopped must not touch the state of the next one.
    a.oncanplay = () => audio === a && (loading = null);
    a.onended = () => audio === a && (playing = null);
    const fail = () => {
      if (audio !== a) return;
      loading = null;
      playing = null;
      error = `${pretty(s.name)} could not be played.`;
    };
    a.onerror = fail;
    playing = s.index;
    a.play().catch(fail);
  }
</script>

<div class="sounds">
  <div class="head">
    <input class="search" placeholder={loaded ? `Search ${list.length} ${noun}` : "Search"} bind:value={filter} />
    {#if group === "music"}<span class="muted">Plays the first minute.</span>{/if}
    {#if selected}
      <span class="muted pick">{pretty(selected.name)}</span>
      <button disabled={busy} onclick={() => replace(selected!)}>Replace with WAV...</button>
      {#if changed.has(selected.index)}<button onclick={() => revert(selected!)}>Revert to game</button>{/if}
    {/if}
  </div>
  {#if error}<p class="bad pad">{error}</p>{/if}
  <div class="scroll">
    <VirtualList items={shown} height={42} key={(s) => s.index} label="Sounds">
      {#snippet row(s)}
        <div
          class="row-item sound"
          role="presentation"
          aria-current={playing === s.index}
          class:picked={selected?.index === s.index}
          onclick={() => (selected = s)}
          oncontextmenu={(e) => ((selected = s), menu(e, s))}
        >
          <button class="play" onclick={() => toggle(s)} aria-label={playing === s.index ? "Stop" : "Play"}>
            {loading === s.index ? "..." : playing === s.index ? "Stop" : "Play"}
          </button>
          <span class="label">{pretty(s.name)}</span>
          <code class="muted name">{s.name}</code>
          {#if changed.has(s.index)}<span class="tag changed">Replaced</span>{/if}
          <span class="tag" title="Sound number">#{s.index}</span>
        </div>
      {/snippet}
      {#snippet empty()}
        <p class="muted pad">{!loaded ? "Loading..." : list.length ? "Nothing matches." : "Nothing here."}</p>
      {/snippet}
    </VirtualList>
  </div>
</div>

<style>
  .sounds {
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
    padding: 0.5rem 1rem 0.5rem;
  }
  .sound .play {
    min-width: 4.2rem;
    padding: 0.2rem 0.6rem;
  }
  .pick {
    max-width: 14rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
  .row-item.picked {
    border-color: var(--outline);
  }
  .pad {
    padding: 1rem;
  }
</style>
