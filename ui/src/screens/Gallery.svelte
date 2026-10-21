<script lang="ts">
  import { onMount } from "svelte";
  import { asset, gameStatus, isReady } from "../lib/backend";

  // Every piece below comes from the user's ROM through the asset bridge.
  // Member numbers and layout facts: docs/ui-assets-inventory.md.

  const SCALE = 3;

  let ready = $state(false);
  let loaded = $state(false);
  let pressed = $state<number | null>(null);
  let menuIndex = $state(0);
  let menuItems = $state<string[]>([]);

  onMount(async () => {
    ready = isReady(await gameStatus());
    loaded = true;
    if (ready) {
      // a/0/0/2 bank 233: the yes/no labels.
      const yes = await fetch(asset("msg/2/233/0")).then((r) => r.text());
      const no = await fetch(asset("msg/2/233/1")).then((r) => r.text());
      menuItems = [yes, no];
    }
  });

  // Menu frame: a/0/0/5 #9 tiles, #3 palette, laid out as BMP_MENUWORK_DrawFrame does.
  const frame = asset("tiles/5:9/5:3/0/3/0,1,2,3,4,5,6,7,8");
  const text = (s: string, colors = "1,2,0") => asset(`text/23:0/23:5/0/${colors}/${s}`);

  // Touch bar buttons (a/0/8/3 #19 palette, #20 tiles, #21 cells, #24 animations):
  // idle and pressed animation numbers per kind, from gTouchBarButtonLooks.
  const buttons = [
    { name: "X", idle: 0, press: 8 },
    { name: "Back", idle: 1, press: 9 },
    { name: "Down", idle: 2, press: 10 },
    { name: "Up", idle: 3, press: 11 },
    { name: "Left", idle: 4, press: 12 },
    { name: "Right", idle: 5, press: 13 },
    { name: "Y", idle: 6, press: 7 },
  ];
  const button = (seq: number) => asset(`anim/83:20/83:19/83:21/83:24/${seq}/0`);

  function onKey(e: KeyboardEvent) {
    if (menuItems.length === 0) return;
    if (e.key === "ArrowDown") menuIndex = (menuIndex + 1) % menuItems.length;
    else if (e.key === "ArrowUp") menuIndex = (menuIndex + menuItems.length - 1) % menuItems.length;
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} onmouseup={() => (pressed = null)} />

<h1>Gallery</h1>

{#if !loaded}
  <p class="muted">Loading...</p>
{:else if !ready}
  <p class="muted">Choose your ROM in Settings to see the game-styled pieces.</p>
{:else}
  <div class="pixel" style="--s: {SCALE}">
    <section>
      <h2>Window and text</h2>
      <div class="window" style="border-image-source: url('{frame}')">
        <img class="text" src={text("Hello! This window, its frame and this font all come from your ROM.")} alt="Hello! This window, its frame and this font all come from your ROM." />
      </div>
    </section>

    <section>
      <h2>Menu list (arrow keys)</h2>
      <div class="window menu" style="border-image-source: url('{frame}')" role="listbox" aria-label="Yes or no">
        {#each menuItems as item, i}
          <div class="item" role="option" aria-selected={i === menuIndex}>
            {#if i === menuIndex}
              <img class="cursor" src={asset("cursor/5:3/0")} alt="" />
            {/if}
            <img class="text" src={text(item)} alt={item} />
          </div>
        {/each}
      </div>
    </section>

    <section>
      <h2>Touch bar buttons (press and hold)</h2>
      <div class="buttons">
        {#each buttons as b, i}
          <button class="sprite" aria-label={b.name} onmousedown={() => (pressed = i)}>
            <img src={button(pressed === i ? b.press : b.idle)} alt="" />
          </button>
        {/each}
      </div>
    </section>

    <section>
      <h2>Background screen</h2>
      <img class="screen" src={asset("screen/27:0/27:1/27:2")} alt="Options menu rows" />
    </section>
  </div>
{/if}

<style>
  .pixel img {
    image-rendering: pixelated;
    zoom: var(--s);
    display: block;
  }
  section {
    margin-bottom: 1.5rem;
  }
  .window {
    display: inline-block;
    border-style: solid;
    border-width: calc(8px * var(--s));
    border-image-slice: 8 fill;
    border-image-width: calc(8px * var(--s));
    border-image-repeat: repeat;
    image-rendering: pixelated;
    padding: calc(2px * var(--s)) calc(4px * var(--s));
  }
  .menu .item {
    display: flex;
    align-items: center;
    gap: calc(2px * var(--s));
    padding-left: calc(12px * var(--s));
    position: relative;
  }
  .menu .cursor {
    position: absolute;
    left: 0;
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .sprite {
    padding: 0;
    border: none;
    background: #2a3240;
    border-radius: 6px;
  }
  .screen {
    max-width: none;
  }
</style>
