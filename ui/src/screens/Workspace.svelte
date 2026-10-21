<script lang="ts">
  import { appStatus, closeProject, isReady, type Status } from "../lib/backend";
  import { ALL, GROUPS, type Category } from "../lib/categories";
  import { play, setSfx, sfxOn } from "../lib/sfx";
  import { forget } from "../lib/cache.svelte";
  import { forgetThumbs } from "../lib/thumbs";
  import ThemePicker from "../lib/ThemePicker.svelte";
  import ChangesDialog from "../lib/ChangesDialog.svelte";
  import { clearEnums } from "../lib/tables";
  import GraphicsBrowser from "../views/GraphicsBrowser.svelte";
  import SoundBrowser from "../views/SoundBrowser.svelte";
  import TextBrowser from "../views/TextBrowser.svelte";
  import FilesBrowser from "../views/FilesBrowser.svelte";
  import CryBrowser from "../views/CryBrowser.svelte";
  import TextureBrowser from "../views/TextureBrowser.svelte";
  import WorldMap from "../views/WorldMap.svelte";
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
  let showChanges = $state(false);
  // Bumped when changes are undone from the list: the open view starts over.
  let reverts = $state(0);
  let reverted = false;

  // The entry a view opens on when another view sends the user there.
  let start = $state<number | undefined>(undefined);

  function pick(c: Category) {
    if (c.id === current.id) return;
    play("select");
    start = undefined;
    current = c;
  }

  function openAt(id: string, index?: number) {
    const c = ALL.find((x) => x.id === id);
    if (!c) return;
    play("select");
    start = index;
    current = c;
  }

  // Groups the user folded away; the one holding the open category stays
  // open. Kept per viewer.
  const FOLD = "kenon.folded";
  let folded = $state<Record<string, boolean>>(
    (() => {
      try {
        return JSON.parse(localStorage.getItem(FOLD) ?? "{}");
      } catch {
        return {};
      }
    })(),
  );
  function fold(label: string) {
    folded[label] = !folded[label];
    play("select");
    try {
      localStorage.setItem(FOLD, JSON.stringify(folded));
    } catch {
      // Not saved; it lasts for this session.
    }
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
  const changeText = $derived(changes === 0 ? "No changes" : `${changes} change${changes === 1 ? "" : "s"}`);
</script>

<div class="workspace">
  <header class="bar">
    <button class="quiet" onclick={() => (play("cancel"), onHome())} title="Back to the start screen">kenon</button>
    <div class="where">
      {#if readOnly}
        <strong>Game data</strong>
        <span class="tag" title="You are looking at the game. Open a project to change it.">read only</span>
      {:else if status.project}
        <strong>{status.project.name}</strong>
        <button class="tag changes" class:changed={changes > 0} onclick={() => ((showChanges = true), play("decide"))} title="See and undo what this project changes">{changeText}</button>
      {/if}
    </div>
    <div class="right">
      <ThemePicker gameOpen={isReady(status)} />
      <button
        class="quiet"
        aria-pressed={sound}
        title="Menu click sounds"
        onclick={() => ((sound = !sound), setSfx(sound), play("select"))}
      >
        {sound ? "Sound on" : "Sound off"}
      </button>
      {#if !readOnly && status.project}
        <button onclick={close} title="Your changes are already saved">Close project</button>
      {/if}
    </div>
  </header>

  <nav class="sidebar" aria-label="Categories">
    {#each GROUPS as group}
      {@const holds = group.items.some((c) => c.id === current.id)}
      {@const shut = !!folded[group.label] && !holds}
      <h3>
        <button class="fold" aria-expanded={!shut} onclick={() => fold(group.label)}>
          <span class="caret" class:open={!shut}></span>{group.label}
        </button>
      </h3>
      {#if !shut}
        {#each group.items as c}
          <button class="row-item" aria-current={c.id === current.id} title={c.label} onclick={() => pick(c)}>
            <span class="label">{c.label}</span>
          </button>
        {/each}
      {/if}
    {/each}
  </nav>

  {#if showChanges}
    <ChangesDialog
      onClose={() => {
        showChanges = false;
        if (reverted) reverts++;
        reverted = false;
      }}
      onChanged={() => {
        reverted = true;
        saved();
      }}
      onRefresh={async () => onStatus(await appStatus())}
    />
  {/if}

  <main class="content">
    {#key `${current.id}/${start}/${reverts}`}
      {#if current.kind === "graphics"}
        <GraphicsBrowser archive={current.archive} {readOnly} {version} onSaved={saved} />
      {:else if current.kind === "sound"}
        <SoundBrowser group={current.group} />
      {:else if current.kind === "text"}
        <TextBrowser archive={current.archive} {readOnly} {version} onSaved={saved} />
      {:else if current.kind === "textures"}
        <TextureBrowser archive={current.archive} noun={current.noun} />
      {:else if current.kind === "worldmap"}
        <WorldMap {readOnly} onOpen={openAt} onSaved={saved} />
      {:else if current.kind === "cries"}
        <CryBrowser />
      {:else if current.kind === "files"}
        <FilesBrowser {version} onGo={openAt} />
      {:else}
        <TableEditor tables={current.tables} {readOnly} {version} {start} onSaved={saved} />
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
    align-items: center;
    flex: none;
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
    margin: 0.9rem 0 0.3rem;
  }
  .fold {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
    padding: 0.15rem 0.6rem;
    border: 2px solid transparent;
    background: transparent;
    box-shadow: none;
    color: inherit;
    font: inherit;
    text-transform: inherit;
    letter-spacing: inherit;
    text-align: left;
  }
  .fold:hover {
    color: var(--ink);
  }
  .caret {
    width: 0;
    height: 0;
    border-left: 5px solid currentColor;
    border-top: 4px solid transparent;
    border-bottom: 4px solid transparent;
    opacity: 0.7;
    transition: transform 0.12s;
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .sidebar h3:first-child {
    margin-top: 0.2rem;
  }
  .changes {
    cursor: pointer;
    box-shadow: none;
  }
  .changes:hover {
    filter: brightness(1.15);
  }
  .content {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
</style>
