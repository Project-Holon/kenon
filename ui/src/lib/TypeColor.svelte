<script lang="ts">
  // A game type's colour (core type_pictures.rs, eidos types.txt `color`):
  // its tag in the interface and the tint of move effects that follow the
  // type (Judgment). Without a line it is the battle move button's shade.
  import { invoke } from "@tauri-apps/api/core";
  import { useParts } from "./parts.svelte";

  let { id, readOnly, version, onChanged }: { id: number; readOnly: boolean; version: number; onChanged: () => void } = $props();

  let saved = $state("#000000");
  let fallback = $state("#000000");
  let value = $state("#000000");
  async function load() {
    const [c, d] = await invoke<[string, string]>("type_color", { id });
    saved = `#${c}`;
    fallback = `#${d}`;
    value = saved;
  }
  $effect(() => {
    void id;
    void version;
    load().catch(() => {});
  });

  const hex = (c: string) => c.replace("#", "").toUpperCase();
  const parts = useParts();
  $effect(() => {
    if (!parts || readOnly) return;
    return parts.register("type-color", {
      dirty: () => hex(value) !== hex(saved),
      save: async () => {
        await invoke("set_type_color", { id, color: hex(value) === hex(fallback) ? null : hex(value) });
        await load();
        onChanged();
      },
      discard: () => (value = saved),
    });
  });
</script>

<section class="type-color">
  <div class="row">
    <label class="field" title="Interface tag and move effects that follow the type (Judgment)">
      <span class="name">Color</span>
      <input type="color" bind:value disabled={readOnly} />
    </label>
    {#if !readOnly && hex(value) !== hex(fallback)}
      <button onclick={() => (value = fallback)} title="The battle move button's color">Reset</button>
    {/if}
  </div>
</section>

<style>
  .type-color {
    margin-top: 0.8rem;
  }
  .row {
    display: flex;
    gap: 0.6rem;
    align-items: end;
  }
  input[type="color"] {
    width: 3rem;
    height: 1.8rem;
    padding: 0;
  }
</style>
