<script lang="ts" module>
  export interface Extras {
    ivs: number[] | null;
    evs: number[] | null;
    nature: number | null;
    shiny: boolean | null;
    friendship: number | null;
    ability: string | null;
  }
</script>

<script lang="ts">
  // A trainer Pokemon's numbers beyond the game's party data (eidos
  // eidos/tables/trainer_mons.txt): IVs, EVs, nature, shiny, friendship and
  // ability. An empty box is the game's own value.

  let {
    extras = $bindable(),
    gameIv,
    natures,
    readOnly,
  }: {
    extras: Extras;
    /// The IV the game gives every stat (strength x 31 / 255).
    gameIv: number;
    natures: string[];
    readOnly: boolean;
  } = $props();

  const STATS = ["HP", "Atk", "Def", "Sp. Atk", "Sp. Def", "Speed"];
  const ABILITY: [string | null, string][] = [
    [null, "By personality"],
    ["1", "First"],
    ["2", "Second"],
    ["hidden", "Hidden"],
  ];

  // One stat of IVs or EVs: an empty box clears the whole set back to the
  // game's once every box is empty.
  function setStat(kind: "ivs" | "evs", i: number, raw: string) {
    const max = kind === "ivs" ? 31 : 255;
    const base = extras[kind] ?? Array(6).fill(kind === "ivs" ? gameIv : 0);
    const next = [...base];
    next[i] = raw === "" ? (kind === "ivs" ? gameIv : 0) : Math.min(max, Math.max(0, Math.round(Number(raw))));
    extras = { ...extras, [kind]: next };
  }
  const evTotal = $derived((extras.evs ?? []).reduce((t, v) => t + v, 0));
</script>

<div class="extras">
  <div class="row">
    <span class="head">IVs</span>
    {#each STATS as s, i}
      <label class="stat">
        <span>{s}</span>
        <input type="number" min="0" max="31" placeholder={String(gameIv)} value={extras.ivs?.[i] ?? ""} readonly={readOnly} onchange={(e) => setStat("ivs", i, e.currentTarget.value)} />
      </label>
    {/each}
    {#if extras.ivs && !readOnly}<button class="quiet" onclick={() => (extras = { ...extras, ivs: null })}>Use IV strength</button>{/if}
  </div>
  <div class="row">
    <span class="head">EVs</span>
    {#each STATS as s, i}
      <label class="stat">
        <span>{s}</span>
        <input type="number" min="0" max="255" placeholder="0" value={extras.evs?.[i] ?? ""} readonly={readOnly} onchange={(e) => setStat("evs", i, e.currentTarget.value)} />
      </label>
    {/each}
    <span class="total" class:bad={evTotal > 510}>{evTotal} / 510</span>
    {#if extras.evs && !readOnly}<button class="quiet" onclick={() => (extras = { ...extras, evs: null })}>Clear</button>{/if}
  </div>
  <div class="row">
    <label class="field">
      <span>Nature</span>
      <select value={extras.nature ?? ""} disabled={readOnly} onchange={(e) => (extras = { ...extras, nature: e.currentTarget.value === "" ? null : Number(e.currentTarget.value) })}>
        <option value="">By personality</option>
        {#each natures as n, i}<option value={i}>{n}</option>{/each}
      </select>
    </label>
    <label class="field small">
      <span>Friendship</span>
      <input type="number" min="0" max="255" placeholder="Species' base" value={extras.friendship ?? ""} readonly={readOnly} onchange={(e) => (extras = { ...extras, friendship: e.currentTarget.value === "" ? null : Math.min(255, Math.max(0, Number(e.currentTarget.value))) })} />
    </label>
    <div class="field">
      <span>Ability</span>
      <div class="seg" role="radiogroup" aria-label="Ability">
        {#each ABILITY as [v, label]}
          <button type="button" role="radio" aria-checked={extras.ability === v} class:on={extras.ability === v} disabled={readOnly} onclick={() => (extras = { ...extras, ability: v })}>{label}</button>
        {/each}
      </div>
    </div>
    <label class="flag">
      <input type="checkbox" checked={!!extras.shiny} disabled={readOnly} onchange={(e) => (extras = { ...extras, shiny: e.currentTarget.checked ? true : null })} />
      Shiny
    </label>
  </div>
</div>

<style>
  .extras {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    padding: 0.5rem 0.6rem;
    margin-left: 1.2rem;
    border-left: 2px solid var(--rim);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 0.5rem 0.8rem;
  }
  .head {
    width: 2.6rem;
    font-weight: 700;
    font-size: 0.8rem;
    color: var(--muted);
    padding-bottom: 0.45rem;
  }
  .stat,
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.8rem;
    color: var(--muted);
  }
  .stat input {
    width: 4.2rem;
  }
  .field select {
    width: 9rem;
  }
  .field.small input {
    width: 6rem;
  }
  .total {
    padding-bottom: 0.45rem;
    font-size: 0.85rem;
    font-variant-numeric: tabular-nums;
  }
  .total.bad {
    color: var(--bad);
  }
  .flag {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding-bottom: 0.45rem;
  }
  .seg {
    display: inline-flex;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    overflow: hidden;
    background: var(--bg);
  }
  .seg button {
    border: none;
    border-radius: 0;
    box-shadow: none;
    background: none;
    padding: 0.3rem 0.6rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
  }
  .seg button + button {
    border-left: 1px solid var(--rim);
  }
  .seg button.on {
    background: var(--accent);
    color: var(--accent-ink);
  }
</style>
