<script lang="ts">
  import { onMount } from "svelte";
  import Start from "./screens/Start.svelte";
  import Workspace from "./screens/Workspace.svelte";
  import { appStatus, isReady, type Status } from "./lib/backend";
  import { enableSfx } from "./lib/sfx";
  import { forgetAll } from "./lib/cache.svelte";
  import { clearEnums } from "./lib/tables";

  let status = $state<Status | null>(null);
  // "view" is the game as it is, read only; "project" edits a mod.
  let screen = $state<"start" | "view" | "project">("start");

  function update(next: Status) {
    // A new game or project: nothing cached still holds.
    if (status && (status.rom !== next.rom || status.project?.root !== next.project?.root)) {
      forgetAll();
      clearEnums();
    }
    status = next;
    enableSfx(isReady(next));
  }

  let failure = $state<string | null>(null);

  onMount(async () => {
    try {
      update(await appStatus());
    } catch (e) {
      failure = String(e);
    }
  });
</script>

{#if status === null}
  <div class="loading" class:muted={!failure} class:bad={!!failure}>{failure ?? "Loading..."}</div>
{:else if screen === "start"}
  <Start
    {status}
    onStatus={update}
    onView={() => (screen = "view")}
    onProject={(s) => {
      update(s);
      screen = "project";
    }}
  />
{:else}
  <Workspace
    {status}
    readOnly={screen === "view"}
    onStatus={update}
    onHome={() => (screen = "start")}
  />
{/if}

<style>
  .loading {
    display: grid;
    place-items: center;
    height: 100%;
  }
</style>
