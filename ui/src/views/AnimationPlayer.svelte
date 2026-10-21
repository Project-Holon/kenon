<script lang="ts">
  import { onMount } from "svelte";
  import { asset, type Ref } from "../lib/backend";
  import { once } from "../lib/cache.svelte";
  import { play as sfx } from "../lib/sfx";

  // Plays a sprite's animations as the game does: each sequence is a list
  // of cells with durations in 60ths of a second, looping from its loop
  // start. Shows the saved version (edits appear after saving).
  let {
    chr,
    pal,
    cer,
    anr,
    offset = 0,
    version = 0,
  }: { chr: Ref; pal: Ref; cer: Ref; anr: Ref; offset?: number; version?: number } = $props();

  interface Frame {
    cell: number;
    duration: number;
  }
  interface Sequence {
    loopStart: number;
    playMode: number;
    frames: Frame[];
  }

  let sequences = $state<Sequence[]>([]);
  let seq = $state(0);
  let frame = $state(0);
  let playing = $state(true);
  let scale = $state(3);
  let error = $state<string | null>(null);

  onMount(() => {
    const key = `animinfo/${anr.join(":")}`;
    once(key, () => fetch(asset(`animinfo/${anr[0]}:${anr[1]}`)).then((r) => r.json()))
      .then((s: Sequence[]) => {
        sequences = s;
        // Open on the first sequence that moves.
        seq = Math.max(0, s.findIndex((q) => q.frames.length > 1));
      })
      .catch((e) => (error = String(e)));
  });

  const current = $derived(sequences[seq]);

  // Advance through the frames by their own durations.
  $effect(() => {
    if (!playing || !current || current.frames.length < 2) return;
    const f = current.frames[Math.min(frame, current.frames.length - 1)];
    const timer = setTimeout(
      () => {
        const next = frame + 1;
        frame = next < current.frames.length ? next : Math.min(current.loopStart, current.frames.length - 1);
      },
      (Math.max(1, f.duration) * 1000) / 60,
    );
    return () => clearTimeout(timer);
  });

  function step(by: number) {
    if (!current) return;
    playing = false;
    sfx("select");
    const n = current.frames.length;
    frame = (frame + by + n) % n;
  }

  function choose(i: number) {
    seq = i;
    frame = 0;
    sfx("select");
  }

  const src = (s: number, f: number) =>
    asset(
      `anim/${chr[0]}:${chr[1]}/${pal[0]}:${pal[1]}/${cer[0]}:${cer[1]}/${anr[0]}:${anr[1]}/${s}/${f}?paloffset=${offset}` +
        (version ? `&v=${Date.now()}` : ""),
    );
</script>

{#if error}
  <p class="bad pad">{error}</p>
{:else if sequences.length}
  <div class="player">
    <div class="frame window">
      {#if current}<img class="px" src={src(seq, frame)} alt="Frame {frame + 1}" style="zoom: {scale}" />{/if}
    </div>
    <div class="controls">
      <label>
        <span class="muted">Animation</span>
        <select value={seq} onchange={(e) => choose(Number((e.currentTarget as HTMLSelectElement).value))}>
          {#each sequences as s, i}<option value={i}>{i} ({s.frames.length} frames)</option>{/each}
        </select>
      </label>
      <div class="buttons">
        <button onclick={() => step(-1)} aria-label="Previous frame">Prev</button>
        <button class="primary" onclick={() => ((playing = !playing), sfx("select"))}>{playing ? "Pause" : "Play"}</button>
        <button onclick={() => step(1)} aria-label="Next frame">Next</button>
      </div>
      <span class="muted">Frame {frame + 1} / {current?.frames.length ?? 0}, cell {current?.frames[frame]?.cell ?? "-"}, {current?.frames[frame]?.duration ?? 0}/60 s</span>
      <label>
        <span class="muted">Size</span>
        <input type="range" min="1" max="6" bind:value={scale} />
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
    min-width: 64px;
    min-height: 64px;
    padding: 0.5rem;
    background: var(--bg);
  }
  .frame img {
    display: block;
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.75rem;
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
