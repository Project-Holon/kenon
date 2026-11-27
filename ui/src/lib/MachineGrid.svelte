<script lang="ts">
  // The TMs, HMs and tutor moves a species can learn, as checkboxes over the
  // bit fields of its record: tmFlags1..4 (one bit per TM/HM slot, TM01 is
  // bit 0 of tmFlags1) and tutorFlags (core/src/battle.rs TUTOR_MOVES).
  import { invoke } from "@tauri-apps/api/core";
  import { moveIcon } from "./icons.svelte";
  import { enumValues, type Value } from "./tables";

  let { values, readOnly }: { values: Value[]; readOnly: boolean } = $props();

  const TUTOR_MOVES = [520, 519, 518, 338, 307, 308, 434];
  let machines = $state<number[]>([]);
  let moves = $state<string[]>([]);
  let filter = $state("");
  $effect(() => {
    invoke<number[]>("machine_moves")
      .then((m) => (machines = m))
      .catch(() => {});
    enumValues("move").then((m) => (moves = m));
  });

  const field = (key: string) => values.find((v) => v.key === key);
  const word = (slot: number) => field(`tmFlags${Math.floor(slot / 32) + 1}`);
  const has = (f: Value | undefined, bit: number) => !!f && ((Number(f.value) >>> bit) & 1) === 1;
  function toggle(f: Value | undefined, bit: number, on: boolean) {
    if (!f) return;
    const mask = (1 << bit) >>> 0;
    f.value = (on ? (Number(f.value) | mask) : (Number(f.value) & ~mask)) >>> 0;
  }
  // Slots 0-94 are TM01-TM95, 95-100 HM01-HM06, and a mod's new TMs follow
  // from TM96 (core/src/machines.rs label). New TMs show after TM95.
  const HM_FIRST = 95;
  const HM_END = 101;
  const label = (slot: number) =>
    slot < HM_FIRST
      ? `TM${String(slot + 1).padStart(2, "0")}`
      : slot < HM_END
        ? `HM${String(slot - 94).padStart(2, "0")}`
        : `TM${String(slot - 5).padStart(2, "0")}`;
  const order = (slot: number) => (slot >= HM_FIRST && slot < HM_END ? slot + 1000 : slot);
  const shown = $derived(
    machines
      .map((mv, slot) => ({ slot, mv }))
      .sort((a, b) => order(a.slot) - order(b.slot))
      .filter(({ slot, mv }) => {
        const q = filter.trim().toLowerCase();
        return !q || label(slot).toLowerCase().includes(q) || (moves[mv] ?? "").toLowerCase().includes(q);
      }),
  );
  const count = $derived(machines.filter((_, slot) => has(word(slot), slot % 32)).length);
  const tutor = $derived(field("tutorFlags"));
</script>

<div class="machines">
  <div class="head">
    <h4>TMs and HMs it can learn ({count} of {machines.length})</h4>
    <input class="search" placeholder="Search TMs or moves" bind:value={filter} aria-label="Search TMs or moves" />
  </div>
  <div class="grid">
    {#each shown as { slot, mv } (slot)}
      <label class="m" class:on={has(word(slot), slot % 32)}>
        <input
          type="checkbox"
          checked={has(word(slot), slot % 32)}
          disabled={readOnly}
          onchange={(e) => toggle(word(slot), slot % 32, (e.currentTarget as HTMLInputElement).checked)}
        />
        <span class="tm">{label(slot)}</span>
        {#if moveIcon(mv)}<img class="px type" src={moveIcon(mv)} alt="" />{/if}
        <span class="name">{moves[mv] ?? mv}</span>
      </label>
    {/each}
  </div>
  {#if tutor}
    <h4>Move tutors</h4>
    <div class="grid">
      {#each TUTOR_MOVES as mv, bit}
        <label class="m" class:on={has(tutor, bit)}>
          <input type="checkbox" checked={has(tutor, bit)} disabled={readOnly} onchange={(e) => toggle(tutor, bit, (e.currentTarget as HTMLInputElement).checked)} />
          {#if moveIcon(mv)}<img class="px type" src={moveIcon(mv)} alt="" />{/if}
          <span class="name">{moves[mv] ?? mv}</span>
        </label>
      {/each}
    </div>
  {/if}
</div>

<style>
  .machines {
    margin-top: 0.9rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--rim);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 1rem;
    justify-content: space-between;
  }
  h4 {
    margin: 0.4rem 0;
    font-size: 0.9rem;
  }
  .search {
    max-width: 14rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
    gap: 0.15rem 0.75rem;
  }
  .m {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.1rem 0.25rem;
    border-radius: var(--radius-s);
    cursor: pointer;
    font-size: 0.9rem;
  }
  .m.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .m input {
    margin: 0;
  }
  .tm {
    width: 2.6rem;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
    font-size: 0.8rem;
  }
  .type {
    height: 12px;
  }
  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
