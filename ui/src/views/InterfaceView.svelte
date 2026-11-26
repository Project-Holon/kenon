<script lang="ts">
  // The game's menus and screens: one entry for their pictures, a row of
  // screens to pick from, then the pictures of the one picked.
  import GraphicsBrowser from "./GraphicsBrowser.svelte";
  import { INTERFACE_SCREENS } from "../lib/categories";

  let {
    screen: wanted,
    readOnly,
    version,
    onSaved,
  }: {
    /// A screen a link asks for (an INTERFACE_SCREENS id).
    screen?: string;
    readOnly: boolean;
    version: number;
    onSaved: () => void;
  } = $props();

  const KEY = "kenon.interface.screen";
  function stored(): string {
    try {
      return localStorage.getItem(KEY) ?? INTERFACE_SCREENS[0].id;
    } catch {
      return INTERFACE_SCREENS[0].id;
    }
  }
  let picked = $state(stored());
  const screen = $derived(INTERFACE_SCREENS.find((s) => s.id === picked) ?? INTERFACE_SCREENS[0]);
  function pick(id: string) {
    picked = id;
    try {
      localStorage.setItem(KEY, id);
    } catch {
      // A remembered screen is a convenience only.
    }
  }
  $effect(() => {
    if (wanted) pick(wanted);
  });
</script>

<div class="interface">
  <nav class="screens" aria-label="Screen">
    {#each INTERFACE_SCREENS as s}
      <button class:on={s.id === screen.id} aria-pressed={s.id === screen.id} onclick={() => pick(s.id)}>{s.label}</button>
    {/each}
  </nav>
  <div class="body">
    {#key screen.archive}
      <GraphicsBrowser archive={screen.archive} {readOnly} {version} {onSaved} />
    {/key}
  </div>
</div>

<style>
  .interface {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    height: 100%;
  }
  .screens {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    padding: 0.6rem 1rem;
    border-bottom: 2px solid var(--outline);
  }
  .screens button {
    padding: 0.25rem 0.65rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
    background: none;
    box-shadow: none;
    border: 1px solid var(--rim);
    border-radius: 999px;
  }
  .screens button.on {
    background: var(--accent);
    color: var(--accent-ink);
    border-color: var(--accent);
  }
  .body {
    min-height: 0;
  }
</style>
