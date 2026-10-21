<script lang="ts">
  import { onMount } from "svelte";
  import { asset } from "../lib/backend";
  import { once } from "../lib/cache.svelte";
  import { thumb } from "../lib/thumbs";
  import { play as sfx } from "../lib/sfx";

  // Plays a battle picture's rig: its parts move as in battle. Frames are
  // made by the back end once and loaded here in order.
  let { path, label = "Battle animation" }: { path: string; label?: string } = $props();

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
  let scale = $state(2);
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

  $effect(() => {
    if (!playing || !info || urls.length < info.frames || info.frames < 2) return;
    const timer = setTimeout(() => (frame = (frame + 1) % info!.frames), (info.durations[frame] * 1000) / 60);
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
  <p class="muted pad">No battle animation for this picture.</p>
{:else if info}
  <div class="player">
    <div class="frame window" style="width: {info.width * scale + 16}px; height: {info.height * scale + 16}px">
      {#if urls[Math.min(frame, urls.length - 1)]}
        <img class="px" src={urls[Math.min(frame, urls.length - 1)]} alt={label} style="width: {info.width * scale}px" />
      {/if}
    </div>
    <div class="controls">
      <strong>{label}</strong>
      {#if info.frames > 1}
        <div class="buttons">
          <button onclick={() => step(-1)}>Prev</button>
          <button class="primary" onclick={() => ((playing = !playing), sfx("select"))}>{playing ? "Pause" : "Play"}</button>
          <button onclick={() => step(1)}>Next</button>
        </div>
        <span class="muted">
          {urls.length < info.frames ? `Loading ${urls.length} / ${info.frames}` : `Frame ${frame + 1} / ${info.frames}`}
        </span>
      {/if}
      <label>
        <span class="muted">Size</span>
        <input type="range" min="1" max="4" bind:value={scale} />
      </label>
    </div>
  </div>
{/if}

<style>
  .player {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1rem;
    padding: 0.6rem 0.9rem;
    border-bottom: 2px solid var(--outline);
  }
  .frame {
    display: grid;
    place-items: center;
    background: var(--bg);
    max-width: 100%;
  }
  .frame img {
    display: block;
    max-width: 100%;
  }
  .controls {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .controls label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .buttons {
    display: flex;
    gap: 0.25rem;
  }
  .pad {
    padding: 0.5rem 1rem;
  }
</style>
