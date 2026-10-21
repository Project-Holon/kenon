<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    dataFolderStatus,
    setDataFolder,
    type DataFolderStatus,
    type DataRootCheck,
  } from "../lib/backend";

  let status = $state<DataFolderStatus | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    status = await dataFolderStatus();
  });

  async function save(folder: string | null) {
    error = null;
    try {
      status = await setDataFolder(folder);
    } catch (err) {
      error = String(err);
    }
  }

  async function choose() {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Choose the game data folder",
      defaultPath: status?.folder ?? status?.suggestion ?? undefined,
    });
    if (typeof picked === "string") await save(picked);
  }

  function describe(check: DataRootCheck): string {
    switch (check.state) {
      case "missing":
        return "This folder does not exist.";
      case "notAFolder":
        return "This is a file, not a folder.";
      case "noGameData":
        return "No game data here. Choose the eidos extraction folder (the one with data/ in it) or its data/ folder.";
      case "ready":
        return "Game data found.";
    }
  }
</script>

<h1>Settings</h1>

<section aria-labelledby="data-folder">
  <h2 id="data-folder">Game data folder</h2>
  <p class="muted">
    The folder eidos unpacked the game into. kenon reads graphics, fonts and sounds from it and
    never changes it. Edits go to a mod folder.
  </p>

  {#if status === null}
    <p class="muted">Loading...</p>
  {:else}
    <dl>
      <dt>Folder</dt>
      <dd><code>{status.folder ?? "not set"}</code></dd>
      {#if status.check}
        <dt>State</dt>
        <dd class:ok={status.check.state === "ready"} class:bad={status.check.state !== "ready"}>
          {describe(status.check)}
        </dd>
        {#if status.check.state === "ready"}
          <dt>NitroFS root</dt>
          <dd><code>{status.check.dataDir}</code></dd>
          <dt>eidos manifest</dt>
          <dd>{status.check.hasManifest ? "present" : "absent (not an eidos extraction)"}</dd>
          <dt>ARM9 image</dt>
          <dd>{status.check.hasArm9 ? "present" : "absent (code tables will be unavailable)"}</dd>
        {/if}
      {/if}
    </dl>

    <div class="row">
      <button class="primary" onclick={choose}>Choose folder...</button>
      {#if status.suggestion && status.suggestion !== status.folder}
        <button onclick={() => save(status!.suggestion)}>Use the eidos folder</button>
      {/if}
      {#if status.folder}
        <button onclick={() => save(null)}>Clear</button>
      {/if}
    </div>
    {#if status.suggestion && status.suggestion !== status.folder}
      <p class="muted">eidos folder found: <code>{status.suggestion}</code></p>
    {/if}
  {/if}

  {#if error}
    <p class="bad" role="alert">{error}</p>
  {/if}
</section>

<style>
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.4rem 1rem;
    margin: 1rem 0;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .ok {
    color: var(--ok);
  }
  .bad {
    color: var(--bad);
  }
</style>
