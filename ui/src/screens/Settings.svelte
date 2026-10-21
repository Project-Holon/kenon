<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { gameStatus, setGame, type GameStatus, type GameCheck } from "../lib/backend";

  let { onChange }: { onChange?: () => void } = $props();

  let status = $state<GameStatus | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    status = await gameStatus();
  });

  async function save(path: string | null) {
    error = null;
    try {
      status = await setGame(path);
      onChange?.();
    } catch (err) {
      error = String(err);
    }
  }

  async function chooseRom() {
    const picked = await open({
      multiple: false,
      title: "Choose your Pokemon Black ROM",
      filters: [{ name: "Nintendo DS ROM", extensions: ["nds"] }],
    });
    if (typeof picked === "string") await save(picked);
  }

  async function chooseFolder() {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Choose a folder unpacked from the ROM",
      defaultPath: status?.suggestion ?? undefined,
    });
    if (typeof picked === "string") await save(picked);
  }

  function describe(check: GameCheck): string {
    switch (check.state) {
      case "rom":
        return check.supported
          ? `Pokemon ${check.identity.game === "black" ? "Black" : "White"} (${check.identity.gameCode})`
          : `Pokemon White (${check.identity.gameCode}). kenon is built against Black; White mostly works.`;
      case "folder":
        return check.hasArm9 ? "Unpacked game folder" : "Unpacked game folder (no ARM9 image: some menus look plain)";
      case "problem":
        return check.message;
    }
  }
</script>

<h1>Settings</h1>

<section aria-labelledby="game">
  <h2 id="game">Game</h2>
  <p class="muted">
    kenon reads graphics, fonts, text and sounds from your own ROM and never changes it. Edits go
    to a mod folder.
  </p>

  {#if status === null}
    <p class="muted">Loading...</p>
  {:else}
    <dl>
      <dt>ROM</dt>
      <dd><code>{status.path ?? "not set"}</code></dd>
      {#if status.check}
        <dt>State</dt>
        <dd class:ok={status.check.state !== "problem"} class:bad={status.check.state === "problem"}>
          {describe(status.check)}
        </dd>
      {/if}
    </dl>

    <div class="row">
      <button class="primary" onclick={chooseRom}>Choose ROM...</button>
      <button onclick={chooseFolder}>Use an unpacked folder...</button>
      {#if status.path}
        <button onclick={() => save(null)}>Clear</button>
      {/if}
    </div>
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
