<script lang="ts">
  import { appStatus, closeProject, isReady, type Status } from "../lib/backend";
  import { ALL, GROUPS, type Category } from "../lib/categories";
  import { play, setSfx, sfxOn } from "../lib/sfx";
  import { forget } from "../lib/cache.svelte";
  import { forgetThumbs } from "../lib/thumbs";
  import ThemePicker from "../lib/ThemePicker.svelte";
  import { clearEnums } from "../lib/tables";
  import GraphicsBrowser from "../views/GraphicsBrowser.svelte";
  import SoundBrowser from "../views/SoundBrowser.svelte";
  import TextBrowser from "../views/TextBrowser.svelte";
  import FilesBrowser from "../views/FilesBrowser.svelte";
  import CryBrowser from "../views/CryBrowser.svelte";
  import TextureBrowser from "../views/TextureBrowser.svelte";
  import TableEditor from "../views/TableEditor.svelte";

  let {
    status,
    readOnly,
    onStatus,
    onHome,
  }: { status: Status; readOnly: boolean; onStatus: (s: Status) => void; onHome: () => void } = $props();

  let current = $state<Category>(ALL.find((c) => c.id === "pokemon-icons")!);
  let sound = $state(sfxOn());
  // Bumped after a save, so views reload what changed.
  let version = $state(0);

  function pick(c: Category) {
    if (c.id === current.id) return;
    play("select");
    current = c;
  }

  async function saved() {
    // Listings carry changed flags and names: ask again after an edit.
    forget("members/", "named/", "entries/");
    forgetThumbs();
    clearEnums();
    version++;
    onStatus(await appStatus());
  }

  async function close() {
    play("cancel");
    onStatus(await closeProject());
    onHome();
  }

  const changes = $derived(status.project?.changes.length ?? 0);
</script>

<div class="workspace">
  <header class="bar">
    <button class="quiet" onclick={() => (play("cancel"), onHome())} title="Back to the start">kenon</button>
    <div class="where">
      {#if readOnly}
        <strong>Game data</strong>
        <span class="tag">read only</span>
      {:else if status.project}
        <strong>{status.project.name}</strong>
        <span class="tag" class:changed={changes > 0}>{changes} changed</span>
      {/if}
    </div>
    <div class="right">
      <ThemePicker gameOpen={isReady(status)} />
      <button class="quiet" aria-pressed={sound} onclick={() => ((sound = !sound), setSfx(sound), play("select"))}>
        {sound ? "Sound on" : "Sound off"}
      </button>
      {#if !readOnly && status.project}
        <button onclick={close}>Close project</button>
      {/if}
    </div>
  </header>

  <nav class="sidebar" aria-label="Categories">
    {#each GROUPS as group}
      <h3>{group.label}</h3>
      {#each group.items as c}
        <button class="row-item" aria-current={c.id === current.id} onclick={() => pick(c)}>
          <span class="label">{c.label}</span>
        </button>
      {/each}
    {/each}
  </nav>

  <main class="content">
    {#key current.id}
      {#if current.kind === "graphics"}
        <GraphicsBrowser archive={current.archive} {readOnly} {version} onSaved={saved} />
      {:else if current.kind === "sound"}
        <SoundBrowser group={current.group} />
      {:else if current.kind === "text"}
        <TextBrowser archive={current.archive} {readOnly} {version} onSaved={saved} />
      {:else if current.kind === "textures"}
        <TextureBrowser archive={current.archive} noun={current.noun} />
      {:else if current.kind === "cries"}
        <CryBrowser />
      {:else if current.kind === "files"}
        <FilesBrowser {version} />
      {:else}
        <TableEditor tables={current.tables} {readOnly} {version} onSaved={saved} />
      {/if}
    {/key}
  </main>
</div>

<style>
  .workspace {
    display: grid;
    grid-template-columns: 15rem minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    height: 100%;
  }
  .bar {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.5rem 1rem;
    background: var(--panel);
    border-bottom: 2px solid var(--outline);
    box-shadow: inset 0 -2px 0 var(--rim);
  }
  .bar > button:first-child {
    font-weight: 800;
    font-size: 1.05rem;
  }
  .where {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
    flex: 1;
  }
  .where strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .right {
    display: flex;
    gap: 0.5rem;
  }
  .sidebar {
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0.75rem 0.6rem 1.5rem;
    background: color-mix(in srgb, var(--panel) 70%, transparent);
    border-right: 2px solid var(--outline);
  }
  .sidebar h3 {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--faint);
    margin: 0.9rem 0.6rem 0.3rem;
  }
  .sidebar h3:first-child {
    margin-top: 0.2rem;
  }
  .content {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
</style>
