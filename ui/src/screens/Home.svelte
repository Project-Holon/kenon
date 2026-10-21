<script lang="ts">
  import { onMount } from "svelte";
  import { dataFolderStatus, type DataFolderStatus } from "../lib/backend";

  let { onOpenSettings }: { onOpenSettings: () => void } = $props();

  let status = $state<DataFolderStatus | null>(null);

  onMount(async () => {
    status = await dataFolderStatus();
  });

  const ready = $derived(status?.check?.state === "ready");
</script>

<h1>kenon</h1>
<p>The modding suite for the native port of Pokemon Black and White.</p>

{#if status === null}
  <p class="muted">Loading the settings...</p>
{:else if ready}
  <p>The game data folder is set. The game-styled theme comes with the asset readers.</p>
{:else}
  <p>
    kenon reads graphics, fonts and sounds from your own unpacked game data. It ships none.
    Until you choose a data folder, it uses this plain theme.
  </p>
  <button class="primary" onclick={onOpenSettings}>Choose the data folder</button>
{/if}
