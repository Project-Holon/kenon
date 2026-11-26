<script lang="ts">
  // A move's battle animation (core/src/move_anims.rs, effscript.rs): which
  // script it plays (eidos tables/move_anims.txt), and its own script, edited
  // as text and saved as the game's binary (a/0/6/6). Preview starts eidos on
  // a copy of the profile's save and loops the animation.
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { enumValues } from "./tables";
  import { play } from "./sfx";
  import { toast } from "morphe/menu.svelte";
  import { debounced } from "morphe/cache.svelte";
  import { useParts } from "./parts.svelte";
  import CodeEditor from "./CodeEditor.svelte";
  import { lastRun } from "./eidosRun";
  import { EFFECT_LANGUAGE, type Marker } from "./monaco";

  let {
    id,
    version,
    readOnly,
    onSaved,
  }: {
    id: number;
    version: number;
    readOnly: boolean;
    onSaved: () => void;
  } = $props();

  /// The last move with an animation of its own in the game.
  const LAST = 559;
  /// eidos's preview Pokémon when none is chosen (Pikachu on Patrat).
  const ATTACKER = 25;
  const DEFENDER = 504;

  let names = $state<string[]>([]);
  let species = $state<string[]>([]);
  enumValues("move").then((n) => (names = n));
  enumValues("species").then((n) => (species = n));

  interface Saved {
    anim: number;
    default: number;
    own: boolean;
  }
  let saved = $state<Saved | null>(null);
  let ownScripts = $state<number[]>([]);
  let pick = $state(0);
  let error = $state("");
  // The move's own script: as saved, and as edited.
  let scriptSaved = $state<string | null>(null);
  let script = $state("");
  let showScript = $state(false);
  let busy = $state(false);

  function reset() {
    pick = saved?.anim ?? 0;
    script = scriptSaved ?? "";
  }
  async function load() {
    const m = id;
    const [a, own] = await Promise.all([invoke<Saved>("move_anim", { id: m }), invoke<number[]>("own_move_scripts")]);
    const text = await invoke<string | null>("move_script", { id: m });
    if (m !== id) return;
    saved = a;
    ownScripts = own;
    scriptSaved = text;
    untrack(reset);
  }
  $effect(() => {
    void version;
    void id;
    showScript = false;
    load().catch((e) => (error = String(e)));
  });
  const animDirty = $derived(!!saved && pick !== saved.anim);
  const scriptDirty = $derived(scriptSaved !== null && script !== scriptSaved);
  const dirty = $derived(animDirty || scriptDirty);

  // Problems while typing.
  let markers = $state<Marker[]>([]);
  const watched = debounced(() => script, 300);
  $effect(() => {
    const text = watched.value;
    if (!showScript) return;
    invoke<[number, string] | null>("check_effect_script", { text })
      .then((p) => (markers = p ? [{ line: p[0], start: 1, end: 200, severity: "error", message: p[1] }] : []))
      .catch(() => {});
  });

  async function save(quiet = false) {
    try {
      if (scriptDirty) await invoke("set_move_script", { id, text: script });
      if (animDirty) await invoke("set_move_anim", { id, anim: pick === saved?.default ? null : pick });
      await load();
      onSaved();
      if (!quiet) {
        play("save");
        toast(`Saved ${names[id] ?? "the move"}'s animation`);
      }
    } catch (e) {
      if (quiet) throw e;
      toast(String(e));
    }
  }

  const entryParts = useParts();
  $effect(() => {
    if (!entryParts || readOnly) return;
    return entryParts.register("anim", {
      dirty: () => dirty,
      save: () => save(true),
      discard: () => reset(),
    });
  });

  async function run(what: () => Promise<void>, done: string) {
    busy = true;
    try {
      await what();
      await load();
      onSaved();
      play("save");
      toast(done);
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }
  /// An added move gets its own script, a copy of the one it plays now.
  function makeOwn() {
    const from = pick;
    run(async () => {
      await invoke("copy_move_script", { id, from });
      showScript = true;
    }, `Created ${nameOf(id)}'s custom script from ${nameOf(from)}`);
  }
  function dropOwn() {
    run(
      () => invoke("drop_move_script", { id }),
      id <= LAST ? `Restored ${nameOf(id)}'s original script` : `Removed ${nameOf(id)}'s custom script`,
    );
  }

  const moves = $derived(
    names
      .map((name, i) => ({ i, name }))
      .filter((m) => m.i > 0 && (m.i <= LAST || ownScripts.includes(m.i)) && m.name)
      .sort((a, b) => a.name.localeCompare(b.name)),
  );
  const pokemon = $derived(
    species
      .map((name, i) => ({ i, name }))
      .filter((s) => s.i > 0 && s.name)
      .sort((a, b) => a.name.localeCompare(b.name)),
  );
  let attacker = $state(ATTACKER);
  let defender = $state(DEFENDER);

  let starting = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  async function preview() {
    try {
      const custom = attacker !== ATTACKER || defender !== DEFENDER;
      lastRun.kind = "preview";
      await invoke("preview_move", { id, species: custom ? [attacker, defender] : null });
      starting = true;
      clearTimeout(timer);
      timer = setTimeout(() => (starting = false), 30000);
    } catch (e) {
      toast(String(e));
    }
  }
  $effect(() => {
    const off = listen<string>("preview-ended", (e) => {
      starting = false;
      if (e.payload && lastRun.kind === "preview") toast(`Preview failed: ${e.payload}`);
    });
    return () => {
      off.then((f) => f());
      clearTimeout(timer);
    };
  });
  const nameOf = (m: number) => names[m] ?? String(m);
  const hasScript = $derived(scriptSaved !== null);
</script>

<div class="window block anim">
  <div class="top">
    <h3>Animation</h3>
    {#if saved && !readOnly}
      <div class="acts">
        {#if hasScript}
          <button class="quiet" onclick={() => (showScript = !showScript)}>{showScript ? "Hide script" : "Edit script"}</button>
        {/if}
        {#if id > LAST && !saved.own}
          <button disabled={busy || dirty} onclick={makeOwn} title={dirty ? "Save first" : `Copies ${nameOf(pick)}'s script`}>Create custom script</button>
        {:else if saved.own}
          <button class="quiet" disabled={busy} onclick={dropOwn}>{id <= LAST ? "Restore original script" : "Remove custom script"}</button>
        {/if}
      </div>
    {/if}
  </div>
  {#if error}
    <p class="bad">{error}</p>
  {:else if saved}
    <div class="row">
      <label class="field">
        <span class="name">Plays</span>
        <select value={String(pick)} disabled={readOnly} onchange={(e) => (pick = Number(e.currentTarget.value))}>
          <option value={String(saved.default)}>{saved.default === id ? "Own script" : nameOf(saved.default)} (default)</option>
          {#each moves.filter((m) => m.i !== saved?.default) as m}
            <option value={String(m.i)}>{m.name}</option>
          {/each}
        </select>
      </label>
      <label class="field">
        <span class="name">Preview attacker</span>
        <select value={String(attacker)} onchange={(e) => (attacker = Number(e.currentTarget.value))}>
          {#each pokemon as s}<option value={String(s.i)}>{s.name}</option>{/each}
        </select>
      </label>
      <label class="field">
        <span class="name">Preview target</span>
        <select value={String(defender)} onchange={(e) => (defender = Number(e.currentTarget.value))}>
          {#each pokemon as s}<option value={String(s.i)}>{s.name}</option>{/each}
        </select>
      </label>
      <button onclick={preview} disabled={dirty} title={dirty ? "Save first" : "Loops in eidos until the window closes"}>Preview</button>
    </div>
    {#if starting}<p class="muted note">Starting eidos. Animation plays in about 30 seconds.</p>{/if}
    {#if id > LAST && !saved.own && pick === saved.default}
      <p class="muted note">Defaults to "Works like" under In battle, otherwise Pound.</p>
    {/if}
    {#if showScript && hasScript}
      {#if pick !== id && saved.default === id}
        <p class="muted note">Unused: this move plays {nameOf(pick)}'s script.</p>
      {/if}
      <div class="editor">
        <CodeEditor bind:value={script} language={EFFECT_LANGUAGE} {markers} readOnly={readOnly} onSave={() => save()} label="Animation script" />
      </div>
    {/if}
  {/if}
</div>

<style>
  .block {
    padding: 0.9rem 1rem;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  h3 {
    flex: 1;
    margin: 0;
  }
  .acts {
    display: flex;
    gap: 0.5rem;
  }
  .row {
    display: flex;
    align-items: flex-end;
    flex-wrap: wrap;
    gap: 0.75rem 1rem;
    margin-top: 0.6rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    width: 14rem;
  }
  .field:first-child {
    width: 18rem;
  }
  .field .name {
    font-size: 0.85rem;
    color: var(--muted);
  }
  .note {
    font-size: 0.9rem;
    margin: 0.5rem 0 0;
  }
  .editor {
    height: 22rem;
    margin-top: 0.7rem;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    overflow: hidden;
  }
  .bad {
    color: var(--bad);
  }
</style>
