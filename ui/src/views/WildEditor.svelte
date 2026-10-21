<script lang="ts">
  import { onMount } from "svelte";
  import { enumValues, saveRecord, tableRecord, type Section, type TableRecord, type Value } from "../lib/tables";
  import { play } from "../lib/sfx";
  import SearchSelect from "../lib/SearchSelect.svelte";
  import { loadIcons, speciesIcon } from "../lib/icons.svelte";
  import { achieved, assignSlots, slotOdds } from "../lib/slots";

  // A map's wild Pokemon, one method at a time: each slot on one line.
  let { index, readOnly, onSaved }: { index: number; readOnly: boolean; onSaved: () => void } = $props();

  type List = Extract<Section, { kind: "list" }>;

  const SEASONS = ["Spring", "Summer", "Autumn", "Winter"];

  // One Pokemon of a method and the share of encounters it should get.
  interface Share {
    species: number;
    form: number;
    min: number;
    max: number;
    want: number;
  }

  let record = $state<TableRecord | null>(null);
  let original = $state("");
  let season = $state(0);
  let method = $state(0);
  let species = $state<string[]>([]);
  let busy = $state(false);
  let error = $state<string | null>(null);
  // "slots": the game's slots one by one; "pokemon": one line per Pokemon
  // with its chance, the slots worked out from it.
  let view = $state<"slots" | "pokemon">("pokemon");
  // "ratio": changing one chance scales the others; "free": type any
  // numbers, applied once they add up to 100.
  let mode = $state<"ratio" | "free">("ratio");
  let shares = $state<Share[]>([]);
  // Free mode numbers not applied yet.
  let pending = $state(false);

  async function load() {
    error = null;
    try {
      const r = await tableRecord("encounters", index, season);
      record = r;
      original = JSON.stringify(r);
      if (method >= r.sections.filter((x) => x.kind === "list").length) method = 0;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    loadIcons();
    load();
    enumValues("species").then((s) => (species = s));
  });

  // The species list is long: build the options once, not per slot.
  const speciesOptions = $derived(species.map((n, k) => [k, n || "None"] as [number, string]));
  const rates = $derived((record?.sections.find((s) => s.kind === "fields") as { values: Value[] } | undefined)?.values ?? []);
  const lists = $derived((record?.sections.filter((s) => s.kind === "list") ?? []) as List[]);
  const rateOf = (i: number) => rates.find((v) => v.key.startsWith("rate") && v.key.endsWith(String(i + 1))) ?? rates[i];
  const used = (l: List) => l.rows.some((r) => r[0] > 0);
  const dirty = $derived(record !== null && JSON.stringify(record) !== original);
  const odds = (l: List, i: number) => `${slotOdds(l.key, l.rows.length)[i]}%`;
  const NAMES: Record<string, string> = {
    grass: "Grass",
    doubleGrass: "Dark grass",
    shakingGrass: "Shaking grass",
    surf: "Surfing",
    surfRipple: "Surf ripples",
    fishing: "Fishing",
    fishingRipple: "Fishing ripples",
  };
  const label = (l: List) => NAMES[l.key] ?? l.label.replace(/ slots$/, "");
  const col = (l: List, name: string) => l.columns.findIndex((c) => c.name === name || c.label.toLowerCase().startsWith(name));

  // The method's Pokemon with what they get now (empty slots left out).
  function readShares(l: List) {
    const [sp, form, min, max] = [col(l, "species"), col(l, "form"), col(l, "min"), col(l, "max")];
    const o = slotOdds(l.key, l.rows.length);
    const out: Share[] = [];
    l.rows.forEach((row, i) => {
      if (row[sp] <= 0) return;
      const f = form >= 0 ? row[form] : 0;
      const s = out.find((x) => x.species === row[sp] && x.form === f);
      if (s) {
        s.want += o[i];
        s.min = Math.min(s.min, row[min]);
        s.max = Math.max(s.max, row[max]);
      } else out.push({ species: row[sp], form: f, min: row[min], max: row[max], want: o[i] });
    });
    shares = out;
    pending = false;
  }

  // Lays the Pokemon out over the slots, as close to their shares as the
  // game's fixed slot odds allow.
  function writeShares(l: List) {
    const [sp, form, min, max] = [col(l, "species"), col(l, "form"), col(l, "min"), col(l, "max")];
    const o = slotOdds(l.key, l.rows.length);
    const live = shares.filter((s) => s.species > 0 && s.want > 0);
    if (!live.length) {
      l.rows.forEach((row) => {
        row[sp] = 0;
        row[min] = 0;
        row[max] = 0;
        if (form >= 0) row[form] = 0;
      });
      return;
    }
    const owner = assignSlots(o, live.map((s) => s.want));
    l.rows.forEach((row, i) => {
      const s = live[owner[i]];
      row[sp] = s.species;
      row[min] = s.min;
      row[max] = Math.max(s.min, s.max);
      if (form >= 0) row[form] = s.form;
    });
    pending = false;
  }

  // What each Pokemon really gets with the slots as they are now.
  const actual = $derived.by(() => {
    const l = lists[method];
    if (!l) return [] as number[];
    const sp = col(l, "species");
    const form = col(l, "form");
    const o = slotOdds(l.key, l.rows.length);
    return shares.map((s) =>
      l.rows.reduce((n, row, i) => n + (row[sp] === s.species && (form < 0 || row[form] === s.form) ? o[i] : 0), 0),
    );
  });
  const total = $derived(Math.round(shares.reduce((n, s) => n + (Number(s.want) || 0), 0) * 10) / 10);

  // Ratio mode: one chance changes, the others keep their ratios to each
  // other and fill the rest.
  function setShare(l: List, k: number, value: number) {
    const v = Math.max(0, Math.min(100, Number(value) || 0));
    if (mode === "free") {
      shares[k].want = v;
      pending = true;
      return;
    }
    const others = shares.reduce((n, s, i) => (i === k ? n : n + s.want), 0);
    const rest = 100 - v;
    shares.forEach((s, i) => {
      if (i === k) s.want = v;
      else s.want = others > 0 ? (s.want / others) * rest : rest / Math.max(1, shares.length - 1);
    });
    writeShares(l);
  }

  function addShare(l: List) {
    shares.push({ species: 0, form: 0, min: 5, max: 5, want: 0 });
    if (mode === "free") pending = true;
    void l;
  }

  function removeShare(l: List, k: number) {
    const gone = shares[k].want;
    shares.splice(k, 1);
    const left = shares.reduce((n, s) => n + s.want, 0);
    if (mode === "ratio" && left > 0) shares.forEach((s) => (s.want += (s.want / left) * gone));
    if (mode === "ratio") writeShares(l);
    else pending = true;
  }

  // Load the per-Pokemon list when the method or the data changes.
  $effect(() => {
    const l = lists[method];
    void original;
    if (l) readShares(l);
  });

  // Levels stay between 0 and 100, and the highest never sits under the lowest.
  function levels(row: number[], lo: number, hi: number, edited: "lo" | "hi") {
    for (const k of [lo, hi]) {
      const n = Math.round(Number(row[k]));
      row[k] = Math.min(100, Math.max(0, Number.isFinite(n) ? n : 0));
    }
    if (row[hi] < row[lo]) row[edited === "lo" ? hi : lo] = row[edited === "lo" ? lo : hi];
  }
  function fixForm(row: number[], at: number) {
    const n = Math.round(Number(row[at]));
    row[at] = Math.min(31, Math.max(0, Number.isFinite(n) ? n : 0));
  }

  function keys(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      if (!readOnly && dirty && !busy && !pending) save();
    }
  }

  async function save() {
    if (!record) return;
    busy = true;
    try {
      const values: Record<string, number> = {};
      const out: Record<string, number[][]> = {};
      for (const s of record.sections) {
        if (s.kind === "fields") s.values.forEach((v) => (values[v.key] = Number(v.value)));
        else out[s.key] = s.rows.map((r) => r.map(Number));
      }
      await saveRecord("encounters", index, { values, lists: out, season });
      play("save");
      onSaved();
      await load();
    } catch (e) {
      error = String(e);
      play("cancel");
    } finally {
      busy = false;
    }
  }
</script>

<div class="wild" role="presentation" onkeydown={keys}>
  <div class="top">
    {#if (record?.seasons ?? 1) > 1}
      <select bind:value={season} onchange={load} aria-label="Season">
        {#each SEASONS.slice(0, record?.seasons ?? 1) as s, i}<option value={i}>{s}</option>{/each}
      </select>
    {/if}
    <div class="spacer"></div>
    {#if !readOnly}
      <button disabled={(!dirty && !pending) || busy} onclick={load}>Discard</button>
      <button class="primary" disabled={!dirty || busy || pending} title={pending ? "The chances must add up to 100%" : ""} onclick={save}>Save</button>
    {/if}
  </div>
  {#if error}<p class="bad">{error}</p>{/if}

  {#if record}
    <div class="methods" role="tablist">
      {#each lists as l, i}
        <button
          role="tab"
          aria-selected={method === i}
          class:primary={method === i}
          class:empty={!used(l)}
          disabled={pending && method !== i}
          onclick={() => (method = i)}>{label(l)}</button>
      {/each}
    </div>

    {#if lists[method]}
      {@const l = lists[method]}
      {@const r = rateOf(method)}
      {@const [sp, form, min, max] = [col(l, "species"), col(l, "form"), col(l, "min"), col(l, "max")]}
      {#if r}
        <label class="rate">
          <span class="muted">Encounter rate</span>
          <input type="number" min={r.min} max={r.max} bind:value={r.value} readonly={readOnly} />
          {#if r.value === 0}<span class="muted">off</span>{/if}
        </label>
      {/if}
      <div class="views">
        <div class="seg" role="radiogroup" aria-label="View">
          <button class:primary={view === "pokemon"} disabled={pending} onclick={() => (view = "pokemon")}>By Pokemon</button>
          <button class:primary={view === "slots"} disabled={pending} onclick={() => (view = "slots")}>By slot</button>
        </div>
        {#if view === "pokemon" && !readOnly}
          <label class="check" title="On: change one chance and the others adjust to keep 100%. Off: type freely, then apply.">
            <input
              type="checkbox"
              checked={mode === "ratio"}
              disabled={pending}
              onchange={(e) => (mode = (e.currentTarget as HTMLInputElement).checked ? "ratio" : "free")}
            />Keep 100% automatically
          </label>
        {/if}
      </div>
      {#if view === "pokemon"}
        <div class="share-head muted">
          <span>Pokemon</span>
          <span>Chance</span>
          <span>Level</span>
          <span></span>
        </div>
        <ul class="shares">
          {#each shares as s, k}
            <li>
              <SearchSelect bind:value={s.species} options={speciesOptions} icon={speciesIcon} disabled={readOnly} label="Pokemon" onchange={() => mode === "ratio" ? writeShares(l) : (pending = true)} />
              <span class="chance">
                <input
                  class="num"
                  type="number"
                  min="0"
                  max="100"
                  value={Math.round(s.want * 10) / 10}
                  readonly={readOnly}
                  onchange={(e) => setShare(l, k, Number((e.currentTarget as HTMLInputElement).value))}
                  aria-label="Chance"
                />%
                {#if !pending && Math.abs((actual[k] ?? 0) - s.want) >= 0.5}
                  <span class="got" title="The game's slots allow this much">{actual[k]}%</span>
                {/if}
              </span>
              <span class="levels">
                <input class="num" type="number" min="0" max="100" bind:value={s.min} readonly={readOnly} onchange={() => (mode === "ratio" ? writeShares(l) : (pending = true))} aria-label="Lowest level" />
                <span class="muted">-</span>
                <input class="num" type="number" min="0" max="100" bind:value={s.max} readonly={readOnly} onchange={() => (mode === "ratio" ? writeShares(l) : (pending = true))} aria-label="Highest level" />
              </span>
              {#if !readOnly}
                <button class="quiet x" onclick={() => removeShare(l, k)} aria-label="Remove">Remove</button>
              {/if}
            </li>
          {:else}
            <li class="muted">No Pokemon here.</li>
          {/each}
        </ul>
        {#if !readOnly}
          <div class="share-foot">
            <button disabled={shares.length >= l.rows.length} onclick={() => addShare(l)}>Add Pokemon</button>
            {#if mode === "free"}
              <span class:bad={total !== 100}>Total {total}%</span>
              <button class="primary" disabled={total !== 100 || !pending} onclick={() => writeShares(l)}>Apply</button>
              {#if pending}<button onclick={() => readShares(l)}>Undo</button>{/if}
            {/if}
          </div>
          <p class="muted small">The game gives each slot fixed odds; the chances snap to what those slots allow.</p>
        {/if}
      {:else}
      <div class="slots-head muted">
        <span>Odds</span>
        <span>Pokemon</span>
        <span>Level</span>
        {#if form >= 0}<span>Form</span>{/if}
      </div>
      <ul class="slots">
        {#each l.rows as row, i}
          <li>
            <span class="pct">{odds(l, i)}</span>
            <SearchSelect bind:value={row[sp]} options={speciesOptions} icon={speciesIcon} disabled={readOnly} label="Pokemon in slot {i + 1}" />
            <span class="levels">
              <input class="num" type="number" min="0" max="100" bind:value={row[min]} onchange={() => levels(row, min, max, "lo")} readonly={readOnly} aria-label="Lowest level" />
              <span class="muted">-</span>
              <input class="num" type="number" min="0" max="100" bind:value={row[max]} onchange={() => levels(row, min, max, "hi")} readonly={readOnly} aria-label="Highest level" />
            </span>
            {#if form >= 0}
              <input class="num" type="number" min="0" max="31" bind:value={row[form]} onchange={() => fixForm(row, form)} readonly={readOnly} aria-label="Form" />
            {/if}
          </li>
        {/each}
      </ul>
      {/if}
    {/if}
  {:else if !error}
    <p class="muted">Loading...</p>
  {/if}
</div>

<style>
  .wild {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .top button {
    padding: 0.2rem 0.6rem;
  }
  .spacer {
    flex: 1;
  }
  .methods {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }
  .methods button {
    padding: 0.15rem 0.5rem;
    font-size: 0.8rem;
  }
  .methods .empty:not(.primary) {
    opacity: 0.5;
  }
  .rate {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
  }
  .rate input {
    width: 4rem;
  }
  .slots {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .slots li,
  .slots-head {
    display: grid;
    grid-template-columns: 2.4rem minmax(0, 1fr) 7.5rem;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
  }
  .slots-head {
    font-size: 0.75rem;
    padding-bottom: 0.1rem;
  }
  .slots-head:has(span:nth-child(4)),
  .slots:has(input[aria-label="Form"]) li {
    grid-template-columns: 2.4rem minmax(0, 1fr) 7.5rem 3rem;
  }
  .views {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }
  .seg {
    display: flex;
    gap: 0.2rem;
  }
  .seg button {
    padding: 0.15rem 0.55rem;
    font-size: 0.8rem;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.8rem;
  }
  .shares {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .shares li,
  .share-head {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 5.2rem 6.5rem auto;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
  }
  .share-head {
    font-size: 0.75rem;
  }
  .chance {
    display: flex;
    align-items: center;
    gap: 0.15rem;
    position: relative;
  }
  .got {
    position: absolute;
    right: 0;
    bottom: -0.8rem;
    font-size: 0.65rem;
    color: var(--muted);
  }
  .x {
    padding: 0.1rem 0.4rem;
    font-size: 0.75rem;
  }
  .share-foot {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.85rem;
  }
  .share-foot button {
    padding: 0.2rem 0.6rem;
  }
  .small {
    font-size: 0.75rem;
    margin: 0;
  }
  .levels {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }
  .pct {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .num {
    width: 100%;
    min-width: 0;
  }
</style>
