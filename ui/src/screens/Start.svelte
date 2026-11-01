<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { join } from "@tauri-apps/api/path";
  import {
    asset,
    forgetRecent,
    gameText,
    importBw,
    isReady,
    newProject,
    openProject,
    recentProjects,
    setRom,
    type NewProject,
    type RecentProject,
    type Status,
  } from "../lib/backend";
  import { gameBadge, gameOf, languageOf, legendaryOf, since } from "../lib/games";
  import { enableSfx, play, setSfx, sfxOn } from "../lib/sfx";
  import { openMenu } from "morphe/menu.svelte";
  import ThemePicker from "../lib/ThemePicker.svelte";
  import NewProjectDialog from "../lib/NewProjectDialog.svelte";

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
  let busy = $state(false);
  let sound = $state(sfxOn());
  let recent = $state<RecentProject[]>([]);
  let spriteOk = $state(true);

  const ready = $derived(isReady(status));
  const identity = $derived(status.check?.state === "rom" ? status.check.identity : null);
  const code = $derived(identity?.gameCode ?? null);
  const kind = $derived(gameOf(code ?? ""));
  const lang = $derived(languageOf(code ?? ""));
  const legendary = $derived(legendaryOf(kind));
  const last = $derived(recent.find((r) => !r.missing) ?? null);

  async function loadRecent() {
    try {
      recent = await recentProjects();
    } catch {
      recent = [];
    }
  }
  onMount(loadRecent);
  $effect(() => {
    void status.recent.length;
    loadRecent();
  });
  $effect(() => {
    void legendary;
    spriteOk = true;
  });

  async function run(task: () => Promise<void>) {
    error = null;
    busy = true;
    try {
      await task();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }

  const chooseRom = () =>
    run(async () => {
      const picked = await open({
        multiple: false,
        title: "Choose your Pokemon Black or White ROM",
        filters: [{ name: "Nintendo DS ROM", extensions: ["nds"] }],
      });
      if (typeof picked !== "string") return;
      const next = await setRom(picked);
      enableSfx(isReady(next));
      onStatus(next);
      play("decide");
    });

  const openRoot = (root: string) =>
    run(async () => {
      play("decide");
      onProject(await openProject(root));
    });

  // A mod folder, or a .bw file unpacked into a new folder beside it.
  const openSomething = () =>
    run(async () => {
      const picked = await open({
        multiple: false,
        title: "Open a project: choose a .bw file, or cancel to pick a folder",
        filters: [{ name: "eidos mod", extensions: ["bw"] }],
      });
      if (typeof picked === "string") {
        const parent = await open({ directory: true, multiple: false, title: "Where should the project folder go?" });
        if (typeof parent !== "string") return;
        const base = picked.split(/[\\/]/).pop()!.replace(/\.bw$/i, "");
        const root = await importBw(picked, await join(parent, base));
        play("decide");
        onProject(await openProject(root));
        return;
      }
      const folder = await open({ directory: true, multiple: false, title: "Open a mod folder" });
      if (typeof folder !== "string") return;
      play("decide");
      onProject(await openProject(folder));
    });

  async function create(root: string, project: NewProject) {
    play("decide");
    const next = await newProject(root, project);
    creating = false;
    onProject(next);
  }

  function recentMenu(e: MouseEvent, r: RecentProject) {
    openMenu(e, [
      { label: "Open", run: () => openRoot(r.root), disabled: !ready || r.missing },
      { label: "Remove from the list", run: async () => (await forgetRecent(r.root), loadRecent()) },
    ]);
  }

  function toggleSound() {
    sound = !sound;
    setSfx(sound);
    play("select");
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && ready && last && !creating) {
      e.preventDefault();
      openRoot(last.root);
    }
  }

  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;
</script>

<svelte:window onkeydown={onKey} />

<div class="start">
  <header>
    <div class="brand">
      {#if ready}
        <img class="px title" src={gameText("kenon")} alt="kenon" />
      {:else}
        <h1>kenon</h1>
      {/if}
      <span class="muted">Modding suite for Pokémon Black and White</span>
    </div>
    <div class="tools">
      <ThemePicker gameOpen={ready} />
      <button class="quiet" onclick={toggleSound} aria-pressed={sound} title="Menu sounds">
        {sound ? "Sound on" : "Sound off"}
      </button>
    </div>
  </header>

  <div class="hero">
    <section
      class="window cart"
      class:black={kind === "black"}
      class:white={kind === "white"}
      class:problem={status.check?.state === "problem"}
    >
      <div class="eyebrow">Your game</div>
      {#if identity && ready}
        <h2 class="game">Pokémon <span>{kind === "black" ? "Black" : "White"}</span></h2>
        <div class="badges">
          {#if lang}<span class="badge">{lang.name}</span>{/if}
          <span class="badge">{identity.gameCode}</span>
          {#if status.check?.state === "rom" && !status.check.supported}
            <span class="badge warn" title="Some pictures and tables may be missing">Unknown cartridge</span>
          {:else if status.romVerified}
            <span class="badge good" title="Matches the known good dump of this cartridge">Verified dump</span>
          {:else if status.romVerified === false}
            <span class="badge warn" title="Not one of the known good dumps: it may be modified or damaged">Not a known dump</span>
          {/if}
        </div>
        <p class="muted note">Everything kenon shows and edits comes from this cartridge.</p>
        <div class="actions">
          <button disabled={busy} onclick={chooseRom}>Change ROM</button>
          <button onclick={() => (play("decide"), onView())}>Browse game data</button>
        </div>
        {#if legendary !== null && spriteOk}
          <img
            class="px sprite"
            src={asset(`preview/4:${20 * legendary}/4:${20 * legendary + 18}?cells=4`)}
            alt={kind === "black" ? "Zekrom" : "Reshiram"}
            onerror={() => (spriteOk = false)}
          />
        {/if}
      {:else if status.check?.state === "folder" && ready}
        <h2 class="game">Unpacked game</h2>
        <p class="muted note">A folder unpacked from the ROM.</p>
        <div class="actions">
          <button disabled={busy} onclick={chooseRom}>Choose a ROM instead</button>
          <button onclick={() => (play("decide"), onView())}>Browse game data</button>
        </div>
      {:else if status.check?.state === "problem"}
        <h2 class="game">This ROM cannot be used</h2>
        <p class="bad note">{status.check.message}</p>
        <div class="actions"><button class="primary" disabled={busy} onclick={chooseRom}>Choose another ROM</button></div>
      {:else}
        <h2 class="game">Choose your ROM</h2>
        <p class="muted note">
          Your own dump of Pokémon Black or White, in any language. kenon reads the game from it and ships no game data.
        </p>
        <div class="actions"><button class="primary big" disabled={busy} onclick={chooseRom}>Choose your ROM</button></div>
      {/if}
    </section>

    <section class="window cont">
      <div class="eyebrow">Continue</div>
      {#if last}
        <h2 class="name">{last.name}</h2>
        {#if last.description}<p class="muted desc">{last.description}</p>{/if}
        <div class="stats">
          <div><b>{last.changes}</b><span>{last.changes === 1 ? "change" : "changes"}</span></div>
          {#if last.opened}<div><b>{since(last.opened)}</b><span>last opened</span></div>{/if}
        </div>
        {#if last.game}
          <div class="badges">
            <span class="gb" class:b={gameOf(last.game) === "black"} class:w={gameOf(last.game) === "white"}
              >{gameBadge(last.game)}</span
            >
            {#if code && last.game !== code}<span class="muted small">Made for another cartridge than the open one.</span>{/if}
          </div>
        {/if}
        <div class="actions">
          <button class="primary big" disabled={!ready || busy} onclick={() => openRoot(last.root)}>Continue</button>
          <span class="muted small">Ctrl+Enter</span>
        </div>
      {:else}
        <h2 class="name">No project yet</h2>
        <p class="muted desc">A project is a mod folder: your changes, rules and code, ready for eidos.</p>
        <div class="actions">
          <button class="primary big" disabled={!ready} onclick={() => ((creating = true), play("select"))}>New project</button>
        </div>
      {/if}
    </section>
  </div>

  <div class="tiles">
    <button class="window tile" disabled={!ready} onclick={() => ((creating = true), play("select"))}>
      <span class="ic accent">+</span>
      <span><b>New project</b><span class="muted">Start a mod for this game.</span></span>
    </button>
    <button class="window tile" disabled={!ready || busy} onclick={openSomething}>
      <span class="ic">⤓</span>
      <span><b>Open a project</b><span class="muted">A mod folder or a .bw file.</span></span>
    </button>
    <button class="window tile" disabled={!ready} onclick={() => (play("decide"), onView())}>
      <span class="ic">▦</span>
      <span><b>Browse game data</b><span class="muted">Look around without changing anything.</span></span>
    </button>
  </div>

  {#if error}<p class="bad" role="alert">{error}</p>{/if}

  {#if recent.length > 0}
    <section class="recent">
      <h3 class="eyebrow">Recent projects</h3>
      <div class="grid">
        {#each recent as r (r.root)}
          <button
            class="window proj"
            disabled={!ready || r.missing}
            title={r.root}
            onclick={() => openRoot(r.root)}
            oncontextmenu={(e) => recentMenu(e, r)}
          >
            <span class="top">
              <b>{r.name}</b>
              {#if r.game}
                <span class="gb" class:b={gameOf(r.game) === "black"} class:w={gameOf(r.game) === "white"}
                  >{gameBadge(r.game)}</span
                >
              {/if}
            </span>
            {#if r.missing}
              <span class="bad d">The folder is gone. Right-click to remove it from the list.</span>
            {:else if r.description}
              <span class="d muted">{r.description}</span>
            {/if}
            <span class="m">
              <span>{r.missing ? "" : plural(r.changes, "change")}</span>
              <span>{since(r.opened)}</span>
            </span>
          </button>
        {/each}
      </div>
    </section>
  {/if}
</div>

{#if creating}
  <NewProjectDialog gameCode={code} onCreate={create} onClose={() => (creating = false)} />
{/if}

<style>
  .start {
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    padding: clamp(1rem, 2.2vw, 2.5rem) clamp(1rem, 3vw, 4rem);
    display: flex;
    flex-direction: column;
    gap: clamp(0.9rem, 1.6vw, 1.75rem);
    max-width: 110rem;
    margin: 0 auto;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    flex-wrap: wrap;
  }
  .brand h1 {
    margin: 0;
  }
  .title {
    height: 45px;
    width: auto;
    display: block;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .eyebrow {
    margin: 0;
    font-size: 0.8rem;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .hero {
    display: grid;
    grid-template-columns: minmax(0, 1.05fr) minmax(0, 1fr);
    gap: clamp(0.9rem, 1.6vw, 1.75rem);
  }
  @media (max-width: 900px) {
    .hero {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .cart,
  .cont {
    position: relative;
    overflow: hidden;
    padding: clamp(1.1rem, 1.8vw, 1.9rem);
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    min-height: 15rem;
  }
  .cart {
    background: linear-gradient(160deg, var(--panel-2), var(--panel));
  }
  .cart.black {
    background:
      radial-gradient(120% 140% at 85% 35%, rgba(47, 143, 232, 0.22), transparent 55%),
      linear-gradient(160deg, var(--panel-2), var(--panel));
  }
  .cart.white {
    background:
      radial-gradient(120% 140% at 85% 35%, rgba(224, 68, 44, 0.2), transparent 55%),
      linear-gradient(160deg, var(--panel-2), var(--panel));
  }
  .cart.problem {
    box-shadow: inset 0 0 0 2px var(--bad);
  }
  .game {
    margin: 0;
    font-size: clamp(1.8rem, 2.6vw, 2.9rem);
    line-height: 1.05;
    position: relative;
    z-index: 1;
  }
  .black .game span {
    color: var(--accent);
  }
  .white .game span {
    color: #ff7a5c;
  }
  .note {
    margin: 0;
    max-width: 36ch;
    position: relative;
    z-index: 1;
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    position: relative;
    z-index: 1;
  }
  .badge {
    border-radius: var(--radius-s);
    padding: 0.15rem 0.6rem;
    font-size: 0.8rem;
    font-weight: 600;
    background: var(--panel-3);
    border: 1px solid var(--rim);
  }
  .badge.good {
    color: var(--ok);
  }
  .badge.warn {
    color: var(--gold);
  }
  .actions {
    margin-top: auto;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6rem;
    position: relative;
    z-index: 1;
  }
  button.big {
    padding: 0.65rem 1.4rem;
    font-size: 1.05rem;
  }
  .sprite {
    position: absolute;
    right: clamp(0.5rem, 2vw, 2.5rem);
    bottom: 0;
    width: clamp(11rem, 17vw, 20rem);
    height: auto;
    filter: drop-shadow(0 0 1.5rem rgba(255, 255, 255, 0.08));
    pointer-events: none;
  }
  .name {
    margin: 0;
    font-size: clamp(1.4rem, 1.9vw, 2rem);
  }
  .desc {
    margin: 0;
    max-width: 60ch;
  }
  .stats {
    display: flex;
    gap: clamp(1rem, 2vw, 2.5rem);
  }
  .stats div {
    display: flex;
    flex-direction: column;
  }
  .stats b {
    font-size: 1.4rem;
  }
  .stats span {
    color: var(--muted);
    font-size: 0.8rem;
  }
  .small {
    font-size: 0.85rem;
  }
  .gb {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 0.1rem 0.45rem;
    border-radius: 5px;
    border: 1px solid var(--rim);
    color: var(--muted);
    white-space: nowrap;
  }
  .gb.w {
    color: #ff9a80;
    border-color: rgba(255, 122, 92, 0.45);
    background: rgba(224, 68, 44, 0.1);
  }
  .gb.b {
    color: #7fc4ff;
    border-color: rgba(47, 143, 232, 0.5);
    background: rgba(47, 143, 232, 0.1);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
    gap: clamp(0.75rem, 1.4vw, 1.5rem);
  }
  .tile {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0.9rem 1.1rem;
    text-align: left;
    font-weight: 400;
    background: var(--panel);
  }
  .tile b {
    display: block;
    font-size: 1rem;
  }
  .tile span span {
    font-size: 0.85rem;
  }
  .ic {
    width: 2.75rem;
    height: 2.75rem;
    flex: none;
    display: grid;
    place-items: center;
    font-size: 1.35rem;
    font-weight: 700;
    border-radius: var(--radius-s);
    border: 2px solid var(--outline);
    background: var(--panel-3);
  }
  .ic.accent {
    background: linear-gradient(var(--accent), var(--accent-dark));
    color: var(--accent-ink);
  }
  .recent {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
    gap: clamp(0.75rem, 1.4vw, 1.4rem);
  }
  .proj {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.8rem 1rem;
    text-align: left;
    font-weight: 400;
    background: var(--panel);
    min-width: 0;
  }
  .top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }
  .top b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .d {
    font-size: 0.85rem;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .m {
    display: flex;
    justify-content: space-between;
    font-size: 0.78rem;
    color: var(--faint);
    margin-top: auto;
  }
</style>
