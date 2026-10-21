<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { join } from "@tauri-apps/api/path";
  import { gameText, isReady, newProject, openProject, setRom, type GameCheck, type Status } from "../lib/backend";
  import { enableSfx, play, setSfx, sfxOn } from "../lib/sfx";
  import ThemePicker from "../lib/ThemePicker.svelte";

  let {
    status,
    onStatus,
    onView,
    onProject,
  }: {
    status: Status;
    onStatus: (s: Status) => void;
    onView: () => void;
    onProject: (s: Status) => void;
  } = $props();

  let error = $state<string | null>(null);
  let creating = $state(false);
  let name = $state("My mod");
  let sound = $state(sfxOn());

  const ready = $derived(isReady(status));

  async function run(task: () => Promise<void>) {
    error = null;
    try {
      await task();
    } catch (e) {
      error = String(e);
      play("cancel");
    }
  }

  const chooseRom = () =>
    run(async () => {
      const picked = await open({
        multiple: false,
        title: "Choose your Pokemon Black ROM",
        filters: [{ name: "Nintendo DS ROM", extensions: ["nds"] }],
      });
      if (typeof picked !== "string") return;
      const next = await setRom(picked);
      enableSfx(isReady(next));
      onStatus(next);
      play("decide");
    });

  const create = () =>
    run(async () => {
      const parent = await open({ directory: true, multiple: false, title: "Where should the mod folder go?" });
      if (typeof parent !== "string") return;
      const folder = name.trim().replace(/[<>:"/\\|?*]/g, "_") || "mod";
      play("decide");
      onProject(await newProject(await join(parent, folder), name.trim() || "mod"));
    });

  const openMod = (root?: string) =>
    run(async () => {
      const picked = root ?? (await open({ directory: true, multiple: false, title: "Open a mod folder" }));
      if (typeof picked !== "string") return;
      play("decide");
      onProject(await openProject(picked));
    });

  function describe(check: GameCheck | null): string {
    if (!check) return "No ROM chosen yet.";
    switch (check.state) {
      case "rom":
        return `Pokemon ${check.identity.game === "black" ? "Black" : "White"} · ${check.identity.gameCode}`;
      case "folder":
        return "An unpacked game folder";
      case "problem":
        return check.message;
    }
  }

  function toggleSound() {
    sound = !sound;
    setSfx(sound);
    play("select");
  }
</script>

<div class="start">
  <header>
    <div class="brand">
      {#if ready}
        <img class="px title" src={gameText("kenon")} alt="kenon" />
      {:else}
        <h1>kenon</h1>
      {/if}
      <p class="muted">Modding for Pokemon Black and White</p>
    </div>
    <div class="tools">
      <ThemePicker />
    <button class="quiet" onclick={toggleSound} aria-pressed={sound} title="Menu sounds">
      {sound ? "Sound on" : "Sound off"}
    </button>
    </div>
  </header>

  <main>
    <section class="window rom" class:problem={status.check?.state === "problem"}>
      <div>
        <h2>Your game</h2>
        <p class:bad={status.check?.state === "problem"} class:muted={status.check?.state !== "problem"}>
          {describe(status.check)}
        </p>
        {#if !ready}
          <p class="muted small">
            kenon reads graphics, text and sound from your own ROM and never changes it. It ships no game
            data.
          </p>
        {/if}
      </div>
      <button class:primary={!ready} onclick={chooseRom}>{ready ? "Change ROM" : "Choose your ROM"}</button>
    </section>

    <div class="cards">
      <section class="window card">
        <h2>New project</h2>
        <p class="muted">Start a mod. Your changes go into a mod folder; the ROM is never touched.</p>
        {#if creating}
          <label>
            <span class="muted small">Name</span>
            <input bind:value={name} onkeydown={(e) => e.key === "Enter" && create()} />
          </label>
          <div class="actions">
            <button class="primary" disabled={!ready} onclick={create}>Choose a folder and create</button>
            <button class="quiet" onclick={() => ((creating = false), play("cancel"))}>Cancel</button>
          </div>
        {:else}
          <button class="primary" disabled={!ready} onclick={() => ((creating = true), play("select"))}>
            New project
          </button>
        {/if}
      </section>

      <section class="window card">
        <h2>View game data</h2>
        <p class="muted">Browse the game's graphics, text, sound and data as they are, read only.</p>
        <button disabled={!ready} onclick={() => (play("decide"), onView())}>View game data</button>
      </section>

      <section class="window card">
        <h2>Open a mod</h2>
        <p class="muted">Keep working on a mod folder you made before.</p>
        <button disabled={!ready} onclick={() => openMod()}>Open a mod folder</button>
      </section>
    </div>

    {#if status.recent.length > 0}
      <section class="window recent">
        <h2>Recent</h2>
        <ul>
          {#each status.recent as root}
            <li>
              <button class="row-item" disabled={!ready} onclick={() => openMod(root)}>
                <span class="label">{root.split(/[\\/]/).pop()}</span>
                <span class="muted small path">{root}</span>
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if error}
      <p class="bad" role="alert">{error}</p>
    {/if}
  </main>
</div>

<style>
  .start {
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 2rem clamp(1rem, 4vw, 3rem);
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .title {
    height: 45px;
    width: auto;
    display: block;
    margin-bottom: 0.25rem;
  }
  main {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .rom {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 1rem 1.25rem;
  }
  .rom > div {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
  }
  .rom.problem {
    box-shadow: inset 0 0 0 2px var(--bad);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
    gap: 1rem;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1.25rem;
  }
  .card p {
    flex: 1;
  }
  .card label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .recent {
    padding: 1rem 1.25rem;
  }
  .recent ul {
    list-style: none;
    margin: 0.5rem 0 0;
    padding: 0;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 50%;
  }
  .small {
    font-size: 0.85rem;
  }
</style>
