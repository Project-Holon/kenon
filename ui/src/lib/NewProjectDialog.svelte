<script lang="ts">
  // Asks what the new project is before it is made: name, description,
  // author, version, the game it is for (the open cartridge) and its folder.
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { join } from "@tauri-apps/api/path";
  import type { NewProject } from "./backend";
  import { gameName, gameOf, languageOf } from "./games";
  import { play } from "./sfx";

  let {
    gameCode,
    onCreate,
    onClose,
  }: {
    gameCode: string | null;
    onCreate: (root: string, project: NewProject) => Promise<void>;
    onClose: () => void;
  } = $props();

  const PARENT = "kenon.newProjectFolder";
  const AUTHOR = "kenon.author";
  const read = (key: string) => {
    try {
      return localStorage.getItem(key) ?? "";
    } catch {
      return "";
    }
  };
  const keep = (key: string, value: string) => {
    try {
      localStorage.setItem(key, value);
    } catch {
      // Not kept; asked again next time.
    }
  };

  let dialog = $state<HTMLDialogElement | null>(null);
  let nameInput = $state<HTMLInputElement | null>(null);
  let name = $state("");
  let description = $state("");
  let author = $state(read(AUTHOR));
  let version = $state("0.1.0");
  let parent = $state(read(PARENT));
  let busy = $state(false);
  let error = $state<string | null>(null);

  const folderName = $derived(name.trim().replace(/[<>:"/\\|?*]/g, "_"));
  const kind = $derived(gameOf(gameCode ?? ""));
  const lang = $derived(languageOf(gameCode ?? ""));

  onMount(() => {
    dialog?.showModal();
    nameInput?.focus();
  });

  async function chooseParent() {
    const picked = await open({ directory: true, multiple: false, title: "Where should the project folder go?" });
    if (typeof picked === "string") parent = picked;
  }

  async function create() {
    if (!name.trim()) {
      nameInput?.focus();
      return;
    }
    error = null;
    if (!parent) await chooseParent();
    if (!parent) return;
    busy = true;
    try {
      keep(PARENT, parent);
      keep(AUTHOR, author.trim());
      await onCreate(await join(parent, folderName), {
        name: name.trim(),
        description: description.trim(),
        author: author.trim(),
        version: version.trim(),
      });
      dialog?.close();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialog}
  class="newproj window"
  onclose={onClose}
  onclick={(e) => {
    if (e.target === dialog) dialog?.close();
  }}
  aria-label="New project"
>
  <header>
    <h2>New project</h2>
    <p class="muted">What are you working on?</p>
  </header>

  <label class="field">
    <span>Name</span>
    <input bind:this={nameInput} bind:value={name} placeholder="Hardcore Unova" onkeydown={(e) => e.key === "Enter" && create()} />
  </label>
  <label class="field">
    <span>What it does <small class="muted">(players see this)</small></span>
    <textarea rows="3" bind:value={description} placeholder="Stronger trainers and a level cap for each badge."></textarea>
  </label>
  <div class="pair">
    <label class="field">
      <span>Author</span>
      <input bind:value={author} />
    </label>
    <label class="field">
      <span>Version</span>
      <input bind:value={version} />
    </label>
  </div>
  <div class="field">
    <span>Game</span>
    <div class="target" class:black={kind === "black"} class:white={kind === "white"}>
      <span class="swatch"></span>
      <div>
        <b>{gameName(kind)}{lang ? ` · ${lang.name}` : ""}</b>
        <div class="muted small">{gameCode ?? "No ROM"}, from your ROM. The project is made for this cartridge.</div>
      </div>
    </div>
  </div>
  <div class="field">
    <span>Folder</span>
    <div class="folder">
      <span class="path" title={parent}>
        {#if parent}{parent}{folderName ? `${parent.includes("\\") ? "\\" : "/"}${folderName}` : ""}{:else}<span class="muted">Not chosen yet</span>{/if}
      </span>
      <button onclick={chooseParent}>Change</button>
    </div>
  </div>
  {#if error}<p class="bad">{error}</p>{/if}

  <footer>
    <button onclick={() => (play("cancel"), dialog?.close())}>Cancel</button>
    <button class="primary" disabled={busy || !name.trim()} onclick={create}>Create project</button>
  </footer>
</dialog>

<style>
  .newproj {
    width: min(38rem, calc(100vw - 2rem));
    max-height: calc(100vh - 4rem);
    padding: 1.25rem 1.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    color: inherit;
    overflow-y: auto;
  }
  .newproj:not([open]) {
    display: none;
  }
  .newproj::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  header h2 {
    margin: 0;
  }
  header p {
    margin: 0.2rem 0 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    min-width: 0;
  }
  .field > span {
    font-size: 0.85rem;
    color: var(--muted);
  }
  textarea {
    resize: vertical;
  }
  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
  }
  .target {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 0.75rem;
    border-radius: var(--radius-s);
    background: var(--panel-2);
    border: 1px solid var(--rim);
  }
  .swatch {
    width: 2.25rem;
    height: 2.25rem;
    border-radius: var(--radius-s);
    flex: none;
    background: var(--panel-3);
    border: 2px solid var(--outline);
  }
  .black .swatch {
    background: linear-gradient(160deg, #3a3f4a, #12141a 60%, #2f8fe8);
  }
  .white .swatch {
    background: linear-gradient(160deg, #f7f6fb, #d9d6e8 55%, #ff8a3d);
  }
  .small {
    font-size: 0.85rem;
  }
  .folder {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    min-width: 0;
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0.35rem 0.55rem;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    background: var(--bg);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
