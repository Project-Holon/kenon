<script lang="ts">
  import { onMount } from "svelte";
  import { asset } from "../lib/backend";
  import { once } from "morphe/cache.svelte";
  import { thumb } from "../lib/thumbs";
  import { play as sfx } from "../lib/sfx";

  // Plays a battle picture's rig: its parts move as in battle. Frames are
  // made by the back end once and loaded here in order.
  import type { Snippet } from "svelte";

  let {
    path,
    label = "Battle animation",
    pause = false,
    onFrame,
    children,
  }: { path: string; label?: string; pause?: boolean; onFrame?: (frame: number) => void; children?: Snippet } = $props();

  interface Info {
    frames: number;
    durations: number[];
    width: number;
    height: number;
  }

  let info = $state<Info | null>(null);
  let urls = $state<string[]>([]);
  let frame = $state(0);
  let playing = $state(true);
  let scale = $state(1);
  let error = $state<string | null>(null);

  onMount(() => {
    const controller = new AbortController();
    once(`riginfo/${path}`, () => fetch(asset(`riginfo/${path}`)).then((r) => (r.ok ? r.json() : Promise.reject(r.statusText))))
      .then(async (i: Info) => {
        info = i;
        // Load the frames in order; play starts once they are in.
        const loaded: string[] = [];
        for (let k = 0; k < i.frames; k++) {
          loaded.push(await thumb(asset(`rigframe/${path}/${k}`), controller.signal));
          if (k === 0) urls = [...loaded];
        }
        urls = loaded;
      })
      .catch((e) => (error = String(e)));
    return () => controller.abort();
  });

  // Tell the editor which frame is showing.
  $effect(() => {
    onFrame?.(frame);
  });

  // While a frame is being edited it must stay put.
  $effect(() => {
    if (pause) playing = false;
  });

  $effect(() => {
    if (!playing || !info || urls.length < info.frames || info.frames < 2) return;
    const timer = setTimeout(() => (frame = (frame + 1) % info!.frames), (Math.max(1, info.durations[frame] ?? 1) * 1000) / 60);
    return () => clearTimeout(timer);
  });

  function step(by: number) {
    if (!info) return;
    playing = false;
    sfx("select");
    frame = (frame + by + info.frames) % info.frames;
  }
</script>

{#if error}
  <p class="muted pad">No battle animation.</p>
{:else if !info}
  <p class="muted pad">Loading...</p>
{:else}
  <div class="player">
    <div class="frame" style="width: {info.width * scale}px; height: {info.height * scale}px">
      {#if urls[Math.min(frame, urls.length - 1)]}
        <img class="px" src={urls[Math.min(frame, urls.length - 1)]} alt={label} style="width: {info.width * scale}px; max-width: 100%; max-height: 14rem; object-fit: contain" />
      {/if}
    </div>
    {#if info.frames > 1}
      <div class="buttons">
        <button onclick={() => step(-1)} aria-label="Previous frame">Prev</button>
        <button class="primary" onclick={() => ((playing = !playing), sfx("select"))}>{playing ? "Pause" : "Play"}</button>
        <button onclick={() => step(1)} aria-label="Next frame">Next</button>
      </div>
      <span class="muted count">
        {urls.length < info.frames ? `${urls.length}/${info.frames}` : `${frame + 1}/${info.frames}`}
      </span>
    {/if}
    <input class="size" type="range" min="1" max="3" bind:value={scale} aria-label="Size" />
    {@render children?.()}
  </div>
{/if}

<style>
  .player {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6rem;
    padding: 0.35rem 0.9rem;
    border-bottom: 2px solid var(--outline);
  }
  .frame {
    display: grid;
    place-items: center;
    max-height: 14rem;
    max-width: 100%;
    overflow: hidden;
    background: var(--bg);
    border-radius: var(--radius-s);
  }
  .frame img {
    display: block;
  }
  .buttons {
    display: flex;
    gap: 0.25rem;
  }
  .buttons button {
    padding: 0.2rem 0.6rem;
  }
  .count {
    font-variant-numeric: tabular-nums;
    min-width: 3.5rem;
  }
  .size {
    width: 5rem;
  }
  .pad {
    padding: 0.4rem 1rem;
  }
</style>
