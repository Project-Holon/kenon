<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { asset } from "../lib/backend";
  import { once } from "../lib/cache.svelte";
  import { tableEntries, type Entry } from "../lib/tables";
  import { play } from "../lib/sfx";
  import ZoneEditor from "./ZoneEditor.svelte";

  // Unova as the game's Town Map draws it, with its cities, routes and
  // places from the map's own table.
  let {
    readOnly,
    onOpen,
    onSaved,
  }: { readOnly: boolean; onOpen: (category: string, index: number) => void; onSaved: () => void } = $props();

  interface Spot {
    zone: number;
    place: string;
    kind: "city" | "route" | "place";
    x: number;
    y: number;
    fly: boolean;
  }
  interface Zone {
    id: number;
    place: string;
    encounters: number | null;
    music: number[];
    trainers: [number, [number, number] | null][];
  }

  const WIDTH = 256;
  const HEIGHT = 168;

  let spots = $state<Spot[]>([]);
  let selected = $state<Spot | null>(null);
  let hover = $state<Spot | null>(null);
  let zones = $state<Zone[]>([]);
  let trainerNames = $state<Entry[]>([]);
  let scale = $state(3);
  let frame = $state<HTMLElement | null>(null);
  let error = $state<string | null>(null);
  // The map open in the editor, with the place's other maps.
  let editing = $state<{ zone: number; zones: { id: number; label: string }[] } | null>(null);
  let audio: HTMLAudioElement | null = null;

  onMount(() => {
    once("spots", () => invoke<Spot[]>("map_spots"))
      .then((s) => (spots = s))
      .catch((e) => (error = String(e)));
    tableEntries("trainers").then((t) => (trainerNames = t)).catch(() => {});
    return () => audio?.pause();
  });

  // The largest whole scale that fits the pane.
  $effect(() => {
    if (!frame) return;
    const observer = new ResizeObserver(() => {
      scale = Math.max(1, Math.min(5, Math.floor((frame!.clientWidth - 16) / WIDTH)));
    });
    observer.observe(frame);
    return () => observer.disconnect();
  });

  async function choose(spot: Spot) {
    play("decide");
    selected = spot;
    zones = [];
    zones = await invoke<Zone[]>("map_place", { zone: spot.zone });
    edit(spot.zone);
  }

  function edit(zone: number) {
    editing = {
      zone,
      zones: zones.map((z, i) => ({ id: z.id, label: i === 0 ? z.place : `${z.place} (map ${z.id})` })),
    };
  }

  function playMusic(seq: number) {
    audio?.pause();
    audio = new Audio(asset(`sound/${seq}?seconds=60`));
    audio.play().catch(() => {});
  }

  const name = (id: number) => trainerNames[id]?.name ?? `Trainer ${id}`;
  const size = (s: Spot) => (s.kind === "city" ? 10 : s.kind === "route" ? 7 : 8);
</script>

{#if editing}
  <ZoneEditor
    zone={editing.zone}
    zones={editing.zones}
    {readOnly}
    {onOpen}
    {onSaved}
    onBack={() => (editing = null)}
  />
{:else}
<div class="world">
  <section class="map" bind:this={frame}>
    {#if error}
      <p class="bad pad">{error}</p>
    {:else}
      <div class="canvas" style="width: {WIDTH * scale}px; height: {HEIGHT * scale}px">
        <img class="px" src={asset("townmap")} alt="Unova" style="width: {WIDTH * scale}px" />
        {#each spots as s (s.zone)}
          <button
            class="spot {s.kind}"
            class:on={selected?.zone === s.zone}
            style="left: {s.x * scale}px; top: {s.y * scale}px; --r: {size(s) * scale * 0.5}px"
            aria-label={s.place}
            onmouseenter={() => (hover = s)}
            onmouseleave={() => (hover = null)}
            onclick={() => choose(s)}
          ></button>
        {/each}
        {#if hover}
          <span class="label window" style="left: {hover.x * scale}px; top: {hover.y * scale}px">{hover.place}</span>
        {/if}
      </div>
    {/if}
  </section>

  <aside class="details">
    {#if !selected}
      <p class="muted pad">Choose a city, route or place.</p>
    {:else}
      <div class="head">
        <h2>{selected.place}</h2>
        <span class="tag">{selected.kind}</span>
        {#if selected.fly}<span class="tag">fly</span>{/if}
      </div>
      <div class="zones">
        {#each zones as z (z.id)}
          <div class="window zone">
            <div class="zone-head">
              <strong>Map {z.id}</strong>
              <button class="quiet" onclick={() => edit(z.id)}>Edit</button>
              {#if z.encounters !== null}
                <button class="quiet" onclick={() => onOpen("encounters", z.encounters!)}>Wild encounters</button>
              {/if}
              {#if z.music[0]}
                <button class="quiet" onclick={() => playMusic(z.music[0])}>Music</button>
              {/if}
            </div>
            {#if z.trainers.length}
              <ul>
                {#each z.trainers as [id, tile]}
                  <li>
                    <button class="row-item" onclick={() => onOpen("trainers", id)}>
                      <span class="label">{name(id)}</span>
                      {#if tile}<span class="muted">{tile[0]}, {tile[1]}</span>{/if}
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        {:else}
          <p class="muted pad">Loading...</p>
        {/each}
      </div>
    {/if}
  </aside>
</div>
{/if}

<style>
  .world {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 20rem;
    height: 100%;
  }
  .map {
    min-width: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 1rem;
    display: flex;
    justify-content: center;
    align-items: flex-start;
  }
  .canvas {
    position: relative;
    flex: none;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    overflow: hidden;
  }
  .canvas img {
    display: block;
  }
  .spot {
    position: absolute;
    width: calc(var(--r) * 2);
    height: calc(var(--r) * 2);
    margin: calc(var(--r) * -1) 0 0 calc(var(--r) * -1);
    padding: 0;
    border-radius: 50%;
    border: 2px solid transparent;
    background: transparent;
    box-shadow: none;
  }
  .spot:hover,
  .spot.on {
    border-color: var(--gold);
    background: color-mix(in srgb, var(--gold) 30%, transparent);
  }
  .label {
    position: absolute;
    transform: translate(-50%, -150%);
    padding: 0.15rem 0.5rem;
    font-size: 0.85rem;
    font-weight: 600;
    white-space: nowrap;
    pointer-events: none;
  }
  .details {
    border-left: 2px solid var(--outline);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .zones {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .zone {
    padding: 0.5rem 0.75rem;
  }
  .zone-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.25rem;
  }
  .zone-head strong {
    margin-right: auto;
  }
  ul {
    list-style: none;
    margin: 0.4rem 0 0;
    padding: 0;
  }
  .pad {
    padding: 1rem;
  }
</style>
