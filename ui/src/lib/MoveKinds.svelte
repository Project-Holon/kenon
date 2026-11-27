<script lang="ts">
  // A move's kinds the later games test and Black and White's record has no
  // flag for (core move_kinds.rs, eidos move_kinds.txt): abilities such as
  // Strong Jaw, Mega Launcher or Bulletproof ask for them.
  import { invoke } from "@tauri-apps/api/core";
  import { useParts } from "./parts.svelte";

  let { id, readOnly }: { id: number; readOnly: boolean } = $props();
  const KINDS: [string, string][] = [
    ["bite", "Bite move"],
    ["pulse", "Pulse move"],
    ["ball", "Ball or bomb move"],
    ["powder", "Powder move"],
    ["one_hit", "Hits once with Parental Bond"],
  ];
  let kinds = $state<string[]>([]);
  let orig = $state("");
  $effect(() => {
    const at = id;
    invoke<string[]>("move_kinds", { id: at })
      .then((k) => {
        if (id !== at) return;
        kinds = k;
        orig = JSON.stringify(k);
      })
      .catch(() => {});
  });
  const parts = useParts();
  $effect(() => {
    if (!parts || readOnly) return;
    return parts.register("kinds", {
      dirty: () => JSON.stringify(kinds) !== orig,
      save: async () => {
        await invoke("set_move_kinds", { id, kinds });
        orig = JSON.stringify(kinds);
      },
      discard: () => (kinds = JSON.parse(orig || "[]")),
    });
  });
  function toggle(k: string, on: boolean) {
    kinds = KINDS.map(([x]) => x).filter((x) => (x === k ? on : kinds.includes(x)));
  }
</script>

<div class="window block kinds">
  <h3>Kinds</h3>
  <div class="row">
    {#each KINDS as [k, label]}
      <label class="flag">
        <input type="checkbox" checked={kinds.includes(k)} disabled={readOnly} onchange={(e) => toggle(k, e.currentTarget.checked)} />
        {label}
      </label>
    {/each}
  </div>
</div>

<style>
  .kinds {
    padding: 0.9rem 1rem;
  }
  h3 {
    margin: 0 0 0.5rem;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 1.4rem;
  }
  .flag {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }
</style>
