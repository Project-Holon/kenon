<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { asset } from "../lib/backend";
  import { once } from "morphe/cache.svelte";
  import { tableEntries, type Entry } from "../lib/tables";
  import { play } from "../lib/sfx";
  import { loadIcons, trainerIcon } from "../lib/icons.svelte";
  import SearchSelect from "morphe/SearchSelect.svelte";
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
  // The hovered spot's button, and where its name floats: fixed to the
  // window, so it is never cut off by the map's edge, and kept on screen.
  let hoverEl = $state<HTMLElement | null>(null);
  let tipEl = $state<HTMLElement | null>(null);
  let tip = $state<{ x: number; y: number; below: boolean } | null>(null);
  function hoverOn(s: Spot, el: HTMLElement) {
    hover = s;
    hoverEl = el;
  }
  function hoverOff() {
    hover = null;
    hoverEl = null;
  }
  // Follows the spot while the map pans or zooms under the pointer.
  $effect(() => {
    const el = hoverEl;
    if (!el) {
      tip = null;
      return;
    }
    let frameId = 0;
    const place = () => {
      const r = el.getBoundingClientRect();
      const half = (tipEl?.offsetWidth ?? 0) / 2;
      const x = Math.min(Math.max(r.left + r.width / 2, half + 8), window.innerWidth - half - 8);
      const below = r.top < 48;
      tip = { x, y: below ? r.bottom + 6 : r.top - 6, below };
      frameId = requestAnimationFrame(place);
    };
    place();
    return () => cancelAnimationFrame(frameId);
  });
  let zones = $state<Zone[]>([]);
  let trainerNames = $state<Entry[]>([]);
  // The largest whole scale that fits the pane, unless the user zoomed.
  let fit = $state(3);
  let zoom = $state<number | null>(null);
  const scale = $derived(zoom ?? fit);
  let placeError = $state<string | null>(null);
  let loadingPlace = $state(false);
  let playingSeq = $state<number | null>(null);
  let frame = $state<HTMLElement | null>(null);
  let error = $state<string | null>(null);
  // The map open in the editor, with the place's other maps.
  let editing = $state<{ zone: number; zones: { id: number; label: string }[] } | null>(null);
  let audio: HTMLAudioElement | null = null;

  onMount(() => {
    loadIcons();
    once("spots", () => invoke<Spot[]>("map_spots"))
      .then((s) => (spots = s))
      .catch((e) => (error = String(e)));
    tableEntries("trainers").then((t) => (trainerNames = t)).catch(() => {});
    return () => audio?.pause();
  });

  $effect(() => {
    if (!frame) return;
    const el = frame;
    const observer = new ResizeObserver(() => {
      const byWidth = Math.floor((el.clientWidth - 32) / WIDTH);
      const byHeight = Math.floor((el.clientHeight - 16) / HEIGHT);
      fit = Math.max(1, Math.min(5, byWidth, byHeight));
    });
    observer.observe(el);
    // Ctrl+wheel zooms; the listener must not be passive to stop the
    // page from zooming too.
    const wheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      const next = Math.max(1, Math.min(8, scale + (e.deltaY < 0 ? 1 : -1)));
      zoom = next === fit ? null : next;
    };
    el.addEventListener("wheel", wheel, { passive: false });
    return () => {
      observer.disconnect();
      el.removeEventListener("wheel", wheel);
    };
  });

  // Middle-drag pans a zoomed map.
  function pan(e: PointerEvent) {
    if (e.button !== 1 || !frame) return;
    e.preventDefault();
    const el = frame;
    let x = e.clientX;
    let y = e.clientY;
    const move = (m: PointerEvent) => {
      el.scrollLeft -= m.clientX - x;
      el.scrollTop -= m.clientY - y;
      x = m.clientX;
      y = m.clientY;
    };
    const stop = () => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", stop);
    };
    addEventListener("pointermove", move);
    addEventListener("pointerup", stop);
  }

  async function choose(spot: Spot) {
    if (loadingPlace) return;
    play("decide");
    selected = spot;
    zones = [];
    placeError = null;
    loadingPlace = true;
    try {
      zones = await invoke<Zone[]>("map_place", { zone: spot.zone });
      edit(spot.zone);
    } catch (e) {
      placeError = String(e);
    } finally {
      loadingPlace = false;
    }
  }

  function edit(zone: number) {
    stopMusic();
    editing = {
      zone,
      zones: zones.map((z, i) => ({ id: z.id, label: i === 0 ? z.place : `${z.place} (map ${z.id})` })),
    };
  }

  function stopMusic() {
    audio?.pause();
    audio = null;
    playingSeq = null;
  }

  function playMusic(seq: number) {
    const again = playingSeq === seq;
    stopMusic();
    if (again) return;
    const a = new Audio(asset(`sound/${seq}?seconds=60`));
    audio = a;
    playingSeq = seq;
    a.onended = () => audio === a && stopMusic();
    a.play().catch(() => audio === a && stopMusic());
  }

  const name = (id: number) => trainerNames[id]?.name ?? `Trainer ${id}`;
  const size = (s: Spot) => (s.kind === "city" ? 10 : s.kind === "route" ? 7 : 8);
</script>

{#if editing}
  <ZoneEditor
    zone={editing.zone}
    zones={editing.zones}
    {readOnly}
    {onSaved}
    onBack={() => (editing = null)}
  />
{:else}
<div class="world">
  <section class="map">
    {#if error}
      <p class="bad pad">{error}</p>
    {/if}
    {#if placeError}<p class="bad pad">{placeError}</p>{/if}
    {#if spots.length}
      <div class="find">
        <SearchSelect
          value={selected?.zone ?? -1}
          label="Find a place"
          options={[[-1, "Find a place..."], ...spots.map((s): [number, string] => [s.zone, s.place])]}
          onchange={(z) => {
            const s = spots.find((x) => x.zone === z);
            if (s) choose(s);
          }}
        />
        <span class="muted">Click a place to edit its maps. Ctrl+wheel zooms, middle-drag moves.</span>
      </div>
    {/if}
    {#if hover && tip}
      <span class="label window" class:below={tip.below} bind:this={tipEl} style="left: {tip.x}px; top: {tip.y}px">{hover.place}</span>
    {/if}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="viewport" bind:this={frame} onpointerdown={pan}>
    {#if !error}
      <div class="canvas" style="width: {WIDTH * scale}px; height: {HEIGHT * scale}px">
        <img class="px" src={asset("townmap")} alt="Unova" style="width: {WIDTH * scale}px" />
        {#each spots as s (s.zone)}
          <button
            class="spot {s.kind}"
            class:on={selected?.zone === s.zone}
            style="left: {s.x * scale}px; top: {s.y * scale}px; --r: {size(s) * scale * 0.5}px"
            aria-label={s.place}
            onmouseenter={(e) => hoverOn(s, e.currentTarget)}
            onmouseleave={hoverOff}
            onfocus={(e) => hoverOn(s, e.currentTarget)}
            onblur={hoverOff}
            onclick={() => choose(s)}
          ></button>
        {/each}
      </div>
    {/if}
    </div>
  </section>

  <aside class="details">
    {#if !selected}
      <p class="muted pad">Pick a city, route or place on the map.</p>
    {:else}
      <div class="head">
        <h2>{selected.place}</h2>
        <span class="tag">{selected.kind}</span>
        {#if selected.fly}<span class="tag" title="You can fly here">fly spot</span>{/if}
      </div>
      <div class="zones">
        {#each zones as z (z.id)}
          <div class="window zone">
            <div class="zone-head">
              <strong>Map {z.id}</strong>
              <button class="quiet" onclick={() => edit(z.id)}>Edit map</button>
              {#if z.encounters !== null}
                <button class="quiet" onclick={() => onOpen("encounters", z.encounters!)}>Wild encounters</button>
              {/if}
              {#if z.music[0]}
                <button class="quiet" onclick={() => playMusic(z.music[0])}>{playingSeq === z.music[0] ? "Stop music" : "Play music"}</button>
              {/if}
            </div>
            {#if z.trainers.length}
              <ul>
                {#each z.trainers as [id, tile]}
                  <li>
                    <button class="row-item" onclick={() => onOpen("trainers", id)}>
                      {#if trainerIcon(id)}<img class="px icon" src={trainerIcon(id)} alt="" />{/if}
                      <span class="label">{name(id)}</span>
                      {#if tile}<span class="muted">{tile[0]}, {tile[1]}</span>{/if}
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        {:else}
          <p class="muted pad">{loadingPlace ? "Loading..." : "No maps."}</p>
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
    min-height: 0;
    padding: 1rem 0 0;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .viewport {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
    padding: 0 1rem 1rem;
  }
  .find {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.75rem;
    margin: 0 auto;
    padding: 0 1rem;
    width: min(100%, 48rem);
  }
  .canvas {
    position: relative;
    flex: none;
    margin: 0 auto;
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
    position: fixed;
    z-index: 20;
    transform: translate(-50%, -100%);
    padding: 0.15rem 0.5rem;
    font-size: 0.85rem;
    font-weight: 600;
    white-space: nowrap;
    pointer-events: none;
  }
  .label.below {
    transform: translate(-50%, 0);
  }
  .icon {
    width: 32px;
    height: 32px;
    flex: none;
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
