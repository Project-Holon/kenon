<script lang="ts">
  import { gameStatus, isReady, type GameStatus } from "../lib/backend";
  import { onMount } from "svelte";

  let { onOpenSettings, onOpenGallery }: { onOpenSettings: () => void; onOpenGallery: () => void } =
    $props();

  let status = $state<GameStatus | null>(null);

  onMount(async () => {
    status = await gameStatus();
  });
</script>

<h1>kenon</h1>
<p>The modding suite for the native port of Pokemon Black and White.</p>

{#if status === null}
  <p class="muted">Loading the settings...</p>
{:else if isReady(status)}
  <p>Your game is set. The component gallery shows the game-styled pieces built so far.</p>
  <button class="primary" onclick={onOpenGallery}>Open the gallery</button>
{:else}
  <p>
    kenon reads graphics, fonts and sounds from your own ROM. It ships none. Until you choose one, it
    uses this plain theme.
  </p>
  <button class="primary" onclick={onOpenSettings}>Choose your ROM</button>
{/if}
