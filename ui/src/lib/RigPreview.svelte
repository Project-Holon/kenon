<script lang="ts">
  import { onMount } from "svelte";
  import { asset } from "./backend";

  // Plays a rig's animation from its bridge path, `rigframe/<members>/0`
  // (with an optional `game/` in front for the game's own version).
  let { path, stamp = 0, alt = "" }: { path: string; stamp?: number; alt?: string } = $props();

  let frames = $state(1);
  let durations = $state<number[]>([]);
  let frame = $state(0);

  const base = $derived(path.replace(/\/\d+$/, ""));
  const info = $derived(base.replace("rigframe/", "riginfo/"));
  const src = (f: number) => asset(`${base}/${f}${stamp ? `?v=${stamp}` : ""}`);

  onMount(() => {
    let stop = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    fetch(asset(info))
      .then((r) => (r.ok ? r.json() : null))
      .then((j) => {
        if (!j || stop) return;
        frames = j.frames;
        durations = j.durations;
        const next = () => {
          if (stop) return;
          timer = setTimeout(() => {
            frame = (frame + 1) % frames;
            next();
          }, Math.max(1, durations[frame] ?? 4) * (1000 / 60));
        };
        if (frames > 1) next();
      })
      .catch(() => {});
    return () => {
      stop = true;
      clearTimeout(timer);
    };
  });
</script>

<!-- Every frame is loaded once; only the current one shows. -->
<span class="rig">
  {#each Array(frames) as _, f}
    <img class="px" class:on={f === frame} src={src(f)} {alt} />
  {/each}
</span>

<style>
  .rig {
    display: grid;
  }
  .rig img {
    grid-area: 1 / 1;
    visibility: hidden;
  }
  .rig img.on {
    visibility: visible;
  }
</style>
