<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { lastRun } from "../lib/eidosRun";
  import { appStatus, closeProject, exportBw, isReady, type Status } from "../lib/backend";
  import { ALL, GROUPS, INTERFACE_SCREENS, type Category } from "../lib/categories";
  import { play, setSfx, sfxOn } from "../lib/sfx";
  import { forget } from "morphe/cache.svelte";
  import { forgetThumbs } from "../lib/thumbs";
  import ThemePicker from "../lib/ThemePicker.svelte";
  import ChangesDialog from "../lib/ChangesDialog.svelte";
  import { clearEnums } from "../lib/tables";
  import GraphicsBrowser from "../views/GraphicsBrowser.svelte";
  import InterfaceView from "../views/InterfaceView.svelte";
  import ParticlesView from "../views/ParticlesView.svelte";
  import SoundBrowser from "../views/SoundBrowser.svelte";
  import TextBrowser from "../views/TextBrowser.svelte";
  import FilesBrowser from "../views/FilesBrowser.svelte";
  import CryBrowser from "../views/CryBrowser.svelte";
  import TextureBrowser from "../views/TextureBrowser.svelte";
  import WorldMap from "../views/WorldMap.svelte";
  import TableEditor from "../views/TableEditor.svelte";
  import AbilitiesView from "../views/AbilitiesView.svelte";
  import TypesView from "../views/TypesView.svelte";
  import MachinesView from "../views/MachinesView.svelte";
  import ShopsView from "../views/ShopsView.svelte";
  import ModSettings from "../views/ModSettings.svelte";
  import ModHooks from "../views/ModHooks.svelte";
  import ModRules from "../views/ModRules.svelte";
  import ModTables from "../views/ModTables.svelte";
  import ModTranslations from "../views/ModTranslations.svelte";
  import { LANGUAGES, textLang, loadLanguages } from "../lib/textLang.svelte";
  import ModCode from "../views/ModCode.svelte";
  import ModConflicts from "../views/ModConflicts.svelte";

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

  // The ROM's language and the reference ROMs, once a game is open.
  $effect(() => {
    if (isReady(status)) loadLanguages().catch(() => {});
  });

  function pick(c: Category) {
    if (c.id === current.id) return;
    play("select");
    start = undefined;
    current = c;
  }
  // The open screen's row stays in view in the sidebar (a link or a saved
  // choice can open one far down the list).
  $effect(() => {
    void current.id;
    queueMicrotask(() =>
      document.querySelector('nav.sidebar [aria-current="true"]')?.scrollIntoView({ block: "nearest" }),
    );
  });

  // Other views ask to open a category ("Open in Files" and the like).
  $effect(() => {
    const go = (e: Event) => {
      const d = (e as CustomEvent<{ category: string; index?: number }>).detail;
      openAt(d.category, d.index);
    };
    window.addEventListener("kenon:open", go);
    return () => window.removeEventListener("kenon:open", go);
  });

  // The Interface screen to show first (an INTERFACE_SCREENS id), when a
  // link names one.
  let interfaceScreen = $state<string | undefined>(undefined);

  function openAt(id: string, index?: number) {
    if (INTERFACE_SCREENS.some((s) => s.id === id)) {
      interfaceScreen = id;
      id = "interface";
    }
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
  // Long groups of picture screens start folded, so the sidebar fits the
  // window; a viewer's own choice wins once made.
  const START_FOLDED = ["Overworld textures"];
  const isFolded = (label: string) => folded[label] ?? START_FOLDED.includes(label);
  function fold(label: string) {
    folded[label] = !isFolded(label);
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

  let exportNote = $state("");
  let exportTimer: ReturnType<typeof setTimeout> | undefined;

  async function exportPackage() {
    play("decide");
    const name = status.project?.name ?? "mod";
    const path = await save({
      defaultPath: `${name}.bw`,
      filters: [{ name: "Mod package", extensions: ["bw"] }],
    });
    if (!path) return;
    try {
      await exportBw(path);
      exportNote = "Exported";
    } catch (e) {
      exportNote = `Export failed: ${e}`;
    }
    clearTimeout(exportTimer);
    exportTimer = setTimeout(() => (exportNote = ""), 5000);
  }

  // Test in game: eidos on the mod's own test save (core preview.rs).
  let testNote = $state("");
  let testTimer: ReturnType<typeof setTimeout> | undefined;
  function note(text: string) {
    testNote = text;
    clearTimeout(testTimer);
    testTimer = setTimeout(() => (testNote = ""), 8000);
  }
  async function testInGame() {
    play("decide");
    try {
      lastRun.kind = "test";
      await invoke("test_in_game");
      note("Starting eidos");
    } catch (e) {
      note(String(e));
    }
  }
  $effect(() => {
    const off = listen<string>("preview-ended", (e) => e.payload && lastRun.kind === "test" && note(`eidos stopped: ${e.payload}`));
    return () => {
      off.then((f) => f());
      clearTimeout(testTimer);
    };
  });

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
      {#if !readOnly && status.project}
        <label class="lang" title="Language for mod text">
          Text language
          <select bind:value={textLang.current}>
            {#each LANGUAGES as [code, name]}<option value={code}>{name}{code === textLang.base ? "" : " (translation)"}</option>{/each}
          </select>
        </label>
      {/if}
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
        {#if exportNote}<span class="tag" role="status">{exportNote}</span>{/if}
        {#if testNote}<span class="tag" role="status">{testNote}</span>{/if}
        <button onclick={testInGame} title="Launches eidos with the saved mod on a separate test save without modifying your saves.">Test in game</button>
        <button onclick={exportPackage} title="Pack this project into one .bw file">Export .bw</button>
        <button onclick={close} title="Your changes are already saved">Close project</button>
      {/if}
    </div>
  </header>

  <nav class="sidebar" aria-label="Categories">
    {#each GROUPS as group}
      {@const holds = group.items.some((c) => c.id === current.id)}
      {@const shut = isFolded(group.label) && !holds}
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
        <GraphicsBrowser archive={current.archive} {start} {readOnly} {version} onSaved={saved} />
      {:else if current.kind === "particles"}
        <ParticlesView {readOnly} {version} onSaved={saved} />
      {:else if current.kind === "interface"}
        <InterfaceView screen={interfaceScreen} {readOnly} {version} onSaved={saved} />
      {:else if current.kind === "sound"}
        <SoundBrowser group={current.group} />
      {:else if current.kind === "text"}
        <TextBrowser archive={current.archive} {readOnly} {version} onSaved={saved} />
      {:else if current.kind === "textures"}
        <TextureBrowser archive={current.archive} noun={current.noun} title={current.label} />
      {:else if current.kind === "abilities"}
        <AbilitiesView {version} {readOnly} onSaved={saved} />
      {:else if current.kind === "types"}
        <TypesView {version} {readOnly} onSaved={saved} />
      {:else if current.kind === "machines"}
        <MachinesView {version} {readOnly} onSaved={saved} />
      {:else if current.kind === "shops"}
        <ShopsView {version} {readOnly} onSaved={saved} />
      {:else if current.kind === "worldmap"}
        <WorldMap {readOnly} onOpen={openAt} onSaved={saved} />
      {:else if current.kind === "cries"}
        <CryBrowser />
      {:else if current.kind === "files"}
        <FilesBrowser {version} onGo={openAt} />
      {:else if current.kind === "mod"}
        {#if current.view === "settings"}
          <ModSettings {readOnly} onSaved={saved} />
        {:else if current.view === "hooks"}
          <ModHooks {readOnly} onSaved={saved} />
        {:else if current.view === "rules"}
          <ModRules {readOnly} onSaved={saved} />
        {:else if current.view === "tables"}
          <ModTables {readOnly} onSaved={saved} />
        {:else if current.view === "translations"}
          <ModTranslations {readOnly} onSaved={saved} />
        {:else if current.view === "code"}
          <ModCode {readOnly} onSaved={saved} />
        {:else}
          <ModConflicts {readOnly} />
        {/if}
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
