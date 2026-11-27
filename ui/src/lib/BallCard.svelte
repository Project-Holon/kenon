<script lang="ts">
  // An item that is a Poke Ball (core balls.rs, eidos balls.txt): the game
  // ball it works like, its catch settings and its own files. A copy of a
  // ball (New) is a ball of its own with copies of the files.
  import { invoke } from "@tauri-apps/api/core";
  import { useParts } from "./parts.svelte";
  import { enumValues } from "./tables";

  let { id, readOnly }: { id: number; readOnly: boolean } = $props();
  interface Ball {
    base: number;
    ball: number | null;
    keys: Record<string, string>;
    particles: number[];
    sprite: number[];
    icon: number[];
  }
  let view = $state<{ ball: Ball; keys: string[]; bases: [number, number][] } | null>(null);
  let base = $state(0);
  let keys = $state<Record<string, string>>({});
  let orig = $state("");
  let items = $state<string[]>([]);
  let types = $state<string[]>([]);
  async function load() {
    const at = id;
    const v = await invoke<typeof view>("item_ball", { item: at });
    if (id !== at) return;
    view = v;
    base = v?.ball.base ?? 0;
    keys = { ...(v?.ball.keys ?? {}) };
    orig = JSON.stringify([base, keys]);
  }
  $effect(() => {
    void id;
    load().catch(() => (view = null));
    enumValues("item").then((n) => (items = n));
    enumValues("type").then((t) => (types = t));
  });
  const parts = useParts();
  $effect(() => {
    if (!parts || readOnly) return;
    return parts.register("ball", {
      dirty: () => !!view && JSON.stringify([base, keys]) !== orig,
      save: async () => {
        await invoke("set_item_ball", { item: id, base, keys });
        await load();
      },
      discard: () => {
        const [b, k] = JSON.parse(orig || "[0,{}]");
        base = b;
        keys = k;
      },
    });
  });
  // The keys a base takes (from the game's rules), shown with their game
  // value as the placeholder.
  const LABEL: Record<string, [string, string]> = {
    ratio: ["Catch rate multiplier", "1"],
    level: ["Below level", "30"],
    per_turn: ["Added each turn", "0.3"],
    turns: ["Within turns", "1"],
    friendship: ["Friendship when caught", ""],
    friendship_bonus: ["Friendship bonus per gain", "0"],
  };
  const set = (k: string, v: string) => (keys = { ...keys, [k]: v });
  // The game's catch rate multiplier by ball id (eidos docs/game-tables.md).
  const RATIO: Record<number, string> = { 2: "2", 3: "1.5", 6: "3", 7: "3.5", 8: "4", 9: "3", 10: "4", 13: "3.5", 15: "5" };
  const baseBall = $derived(view?.bases.find(([item]) => item === base)?.[1] ?? 0);
  const hint = (k: string) => (k === "ratio" ? (RATIO[baseBall] ?? "1") : k === "friendship_bonus" && baseBall === 11 ? "1" : LABEL[k][1]);
  const typeWords = $derived(types.map((t) => t.toLowerCase()));
</script>

{#if view}
  <div class="window block ball">
    <h3>Poké Ball</h3>
    <div class="grid">
      <label class="field">
        <span class="name">Works like</span>
        <select value={String(base)} disabled={readOnly} onchange={(e) => ((base = Number(e.currentTarget.value)), (keys = {}))}>
          {#each view.bases as [item]}<option value={String(item)}>{items[item] ?? item}</option>{/each}
        </select>
      </label>
      {#if view.ball.ball}
        <span class="field"><span class="name">Ball number</span><span class="value">{view.ball.ball}</span></span>
      {/if}
      {#each view.keys as k}
        {#if k === "heal"}
          <label class="flag"><input type="checkbox" checked={keys.heal === "1"} disabled={readOnly} onchange={(e) => set("heal", e.currentTarget.checked ? "1" : "")} /> Heals when caught</label>
        {:else if k === "type" || k === "type.2"}
          <label class="field">
            <span class="name">{k === "type" ? "Works best on type" : "Or type"}</span>
            <select value={keys[k] ?? ""} disabled={readOnly} onchange={(e) => set(k, e.currentTarget.value)}>
              <option value="">{types[k === "type" ? 10 : 6] ?? ""} (default)</option>
              {#each typeWords as t}<option value={t}>{types[typeWords.indexOf(t)]}</option>{/each}
            </select>
          </label>
        {:else if LABEL[k]}
          <label class="field">
            <span class="name">{LABEL[k][0]}</span>
            <input type="number" step="any" value={keys[k] ?? ""} placeholder={hint(k)} readonly={readOnly} onchange={(e) => set(k, e.currentTarget.value)} />
          </label>
        {/if}
      {/each}
    </div>
    {#if view.ball.particles.length}
      <p class="muted note">
        Own files: particles {view.ball.particles.join(", ")}, ball sprite {view.ball.sprite.join(", ")}, icon {view.ball.icon.join(", ")}.
      </p>
    {/if}
  </div>
{/if}

<style>
  .ball {
    padding: 0.9rem 1rem;
  }
  h3 {
    margin: 0 0 0.6rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(14rem, 1fr));
    gap: 0.6rem 1rem;
    align-items: end;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .name {
    color: var(--muted);
    font-size: 0.85rem;
  }
  .value {
    padding: 0.4rem 0;
  }
  .flag {
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }
  .note {
    margin: 0.7rem 0 0;
    font-size: 0.85rem;
  }
</style>
