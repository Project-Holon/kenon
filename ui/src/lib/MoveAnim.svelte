<script lang="ts">
  // A move's battle animation: one of the game's moves (1..559), eidos
  // tables/move_anims.txt (core/src/move_anims.rs). Preview starts eidos on
  // a copy of the profile's save and loops the animation.
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { enumValues } from "./tables";
  import { play } from "./sfx";
  import { toast } from "morphe/menu.svelte";
  import { useParts } from "./parts.svelte";

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

  /// The last move with an animation of its own.
  const LAST = 559;
  /// eidos's preview Pokémon when none is chosen (Pikachu on Patrat).
  const ATTACKER = 25;
  const DEFENDER = 504;

  let names = $state<string[]>([]);
  let species = $state<string[]>([]);
  enumValues("move").then((n) => (names = n));
  enumValues("species").then((n) => (species = n));

  let saved = $state<{ anim: number; default: number } | null>(null);
  let pick = $state(0);
  let error = $state("");
  function reset() {
    pick = saved?.anim ?? 0;
  }
  $effect(() => {
    void version;
    const m = id;
    invoke<{ anim: number; default: number }>("move_anim", { id: m })
      .then((a) => {
        saved = a;
        untrack(reset);
      })
      .catch((e) => (error = String(e)));
  });
  const dirty = $derived(!!saved && pick !== saved.anim);

  async function save(quiet = false) {
    try {
      await invoke("set_move_anim", { id, anim: pick === saved?.default ? null : pick });
      saved = await invoke("move_anim", { id });
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

  const moves = $derived(
    names
      .map((name, i) => ({ i, name }))
      .filter((m) => m.i > 0 && m.i <= LAST && m.name)
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
      if (e.payload) toast(`Preview failed: ${e.payload}`);
    });
    return () => {
      off.then((f) => f());
      clearTimeout(timer);
    };
  });
  const nameOf = (m: number) => names[m] ?? String(m);
</script>

<div class="window block anim">
  <h3>Animation</h3>
  {#if error}
    <p class="bad">{error}</p>
  {:else if saved}
    <div class="row">
      <label class="field">
        <span class="name">Plays</span>
        <select value={String(pick)} disabled={readOnly} onchange={(e) => (pick = Number(e.currentTarget.value))}>
          <option value={String(saved.default)}>{nameOf(saved.default)} (default)</option>
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
    {#if id > LAST && pick === saved.default}
      <p class="muted note">Defaults to "Works like" under In battle, otherwise Pound.</p>
    {/if}
  {/if}
</div>

<style>
  .block {
    padding: 0.9rem 1rem;
  }
  h3 {
    margin: 0;
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
  .bad {
    color: var(--bad);
  }
</style>
