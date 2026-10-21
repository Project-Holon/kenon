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
  // "pokemon": one line per Pokemon with its chance; "slots": the game's
  // slots one by one.
  let view = $state<"slots" | "pokemon">("pokemon");
  let shares = $state<Share[]>([]);
  // The Pokemon that clicks on the slot bar hand slots to.
  let brush = $state(0);

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

  // The method's Pokemon and the chance their slots give them now.
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
    if (brush >= out.length) brush = 0;
  }

  // Lays the Pokemon out over the slots for the wanted chances, as close as
  // the game's fixed slot odds allow, then reads back what they got.
  function layOut(l: List, wanted: number[]) {
    const [sp, form, min, max] = [col(l, "species"), col(l, "form"), col(l, "min"), col(l, "max")];
    const o = slotOdds(l.key, l.rows.length);
    const live = shares.map((s, k) => ({ s, want: wanted[k] ?? 0 })).filter((x) => x.want > 0);
    if (!live.length) return;
    const owner = assignSlots(o, live.map((x) => x.want));
    l.rows.forEach((row, i) => {
      const { s } = live[owner[i]];
      row[sp] = s.species;
      row[min] = s.min;
      row[max] = Math.max(s.min, s.max);
      if (form >= 0) row[form] = s.form;
    });
    const keep = shares.filter((s) => s.species <= 0);
    readShares(l);
    shares.push(...keep);
  }

  // Wanted chances when Pokemon k asks for `value`: the others keep their
  // ratios to each other and fill the rest.
  function scaled(k: number, value: number): number[] {
    const others = shares.reduce((n, s, i) => (i === k ? n : n + s.want), 0);
    const rest = 100 - value;
    const free = shares.filter((s, i) => i !== k && s.species > 0).length;
    return shares.map((s, i) =>
      i === k ? value : s.species <= 0 ? 0 : others > 0 ? (s.want / others) * rest : rest / Math.max(1, free),
    );
  }

  // One step up or down: the next chance the slots can give this Pokemon.
  function step(l: List, k: number, dir: 1 | -1) {
    const now = shares[k].want;
    const o = slotOdds(l.key, l.rows.length);
    for (const d of [...new Set(o)].sort((a, b) => a - b).concat([25, 30, 50])) {
      const want = Math.max(0, Math.min(100, now + dir * d));
      const w = scaled(k, want);
      const live = w.map((x, i) => ({ x, i })).filter((v) => v.x > 0 && shares[v.i].species > 0);
      if (!live.length) continue;
      const owner = assignSlots(o, live.map((v) => v.x));
      const got = achieved(o, owner, live.length);
      const mine = live.findIndex((v) => v.i === k);
      const result = mine < 0 ? 0 : got[mine];
      if (dir > 0 ? result > now : result < now) {
        layOut(l, w);
        play("select");
        return;
      }
    }
  }

  // Splits the slots as evenly as they allow.
  function even(l: List) {
    const n = shares.filter((s) => s.species > 0).length;
    if (!n) return;
    layOut(l, shares.map((s) => (s.species > 0 ? 100 / n : 0)));
    play("decide");
  }

  // A new Pokemon (one not on the list yet, to change from there) takes
  // the smallest slot of whoever holds the most.
  function addShare(l: List) {
    let pick = 1;
    while (shares.some((x) => x.species === pick)) pick++;
    shares.push({ species: pick, form: 0, min: shares[0]?.min ?? 5, max: shares[0]?.max ?? 5, want: 0 });
    brush = shares.length - 1;
    giveSmallest(l, brush);
  }

  function giveSmallest(l: List, k: number) {
    const sp = col(l, "species");
    const o = slotOdds(l.key, l.rows.length);
    const counts = new Map<number, number>();
    l.rows.forEach((row) => counts.set(row[sp], (counts.get(row[sp]) ?? 0) + 1));
    const order = o.map((_, i) => i).sort((a, b) => o[a] - o[b]);
    const slot = order.find((i) => l.rows[i][sp] <= 0) ?? order.find((i) => (counts.get(l.rows[i][sp]) ?? 0) > 1);
    if (slot !== undefined) paint(l, slot, k);
  }

  // Gives one slot to Pokemon k.
  function paint(l: List, slot: number, k: number) {
    const s = shares[k];
    if (!s) return;
    const [sp, form, min, max] = [col(l, "species"), col(l, "form"), col(l, "min"), col(l, "max")];
    const row = l.rows[slot];
    row[sp] = s.species;
    row[min] = s.min;
    row[max] = Math.max(s.min, s.max);
    if (form >= 0) row[form] = s.form;
    const keep = shares.filter((x) => x.species <= 0);
    const at = shares[k];
    readShares(l);
    shares.push(...keep);
    brush = Math.max(0, shares.findIndex((x) => x.species === at.species && x.form === at.form));
  }

  // A removed Pokemon's slots go to the others, keeping their ratios.
  function removeShare(l: List, k: number) {
    const w = scaled(k, 0);
    shares.splice(k, 1);
    w.splice(k, 1);
    if (!shares.some((s) => s.species > 0)) {
      const [sp, min, max] = [col(l, "species"), col(l, "min"), col(l, "max")];
      l.rows.forEach((row) => ((row[sp] = 0), (row[min] = 0), (row[max] = 0)));
      shares = [];
      return;
    }
    layOut(l, w);
  }

  // Species or levels of a Pokemon changed: rewrite its slots.
  function editShare(l: List, k: number, before: { species: number; form: number }) {
    const s = shares[k];
    s.max = Math.max(s.min, s.max);
    const [sp, form, min, max] = [col(l, "species"), col(l, "form"), col(l, "min"), col(l, "max")];
    for (const row of l.rows) {
      if (row[sp] !== before.species || (form >= 0 && row[form] !== before.form)) continue;
      row[sp] = s.species;
      row[min] = s.min;
      row[max] = s.max;
      if (form >= 0) row[form] = s.form;
    }
    if (before.species <= 0) giveSmallest(l, k);
    else readShares(l);
  }

  // Colours that tell the Pokemon apart on the slot bar.
  const HUES = [205, 25, 130, 280, 50, 340, 175, 0, 90, 240, 310, 155];
  const hue = (k: number) => HUES[k % HUES.length];
  const ownerOf = (l: List, i: number) => {
    const [sp, form] = [col(l, "species"), col(l, "form")];
    return shares.findIndex((s) => s.species === l.rows[i][sp] && (form < 0 || s.form === l.rows[i][form]));
  };

  // Reread when the method or the data changes.
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
      if (!readOnly && dirty && !busy) save();
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
      <button disabled={!dirty || busy} onclick={load}>Discard</button>
      <button class="primary" disabled={!dirty || busy} onclick={save}>Save</button>
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
          <button class:primary={view === "pokemon"} onclick={() => (view = "pokemon")}>By Pokemon</button>
          <button class:primary={view === "slots"} onclick={() => (view = "slots")}>By slot</button>
        </div>
      </div>
      {#if view === "pokemon"}
        {@const o = slotOdds(l.key, l.rows.length)}
        <!-- The game's slots, sized by their odds and coloured by who has them. -->
        <div class="bar" role="group" aria-label="Slots">
          {#each l.rows as _, i}
            {@const k = ownerOf(l, i)}
            <button
              class="slot"
              style="flex: {o[i]}; --h: {k < 0 ? 0 : hue(k)}"
              class:none={k < 0}
              disabled={readOnly || !shares[brush]}
              title="{o[i]}%: {k < 0 ? 'empty' : species[shares[k].species] || 'Pokemon'}"
              onclick={() => (paint(l, i, brush), play("select"))}
            >
              {#if k >= 0 && o[i] >= 10 && speciesIcon(shares[k].species)}<img class="px" src={speciesIcon(shares[k].species)} alt="" />{/if}
            </button>
          {/each}
        </div>
        {#if !readOnly && shares.length}
          <p class="muted small">Pick a Pokemon below, then click slots to give them to it.</p>
        {/if}
        <ul class="shares">
          {#each shares as s, k}
            <li class:brush={brush === k && !readOnly}>
              <button class="swatch" style="--h: {hue(k)}" aria-label="Give slots to this Pokemon" aria-pressed={brush === k} disabled={readOnly} onclick={() => (brush = k)}></button>
              <SearchSelect
                value={s.species}
                options={speciesOptions}
                icon={speciesIcon}
                disabled={readOnly}
                label="Pokemon"
                onchange={(v) => {
                  const before = { species: s.species, form: s.form };
                  s.species = v;
                  editShare(l, k, before);
                }}
              />
              <span class="chance">
                {#if !readOnly}<button class="step" onclick={() => step(l, k, -1)} disabled={s.want <= 0} aria-label="Less often">-</button>{/if}
                <strong>{s.want}%</strong>
                {#if !readOnly}<button class="step" onclick={() => step(l, k, 1)} disabled={s.want >= 100} aria-label="More often">+</button>{/if}
              </span>
              <span class="levels">
                <input class="num" type="number" min="0" max="100" bind:value={s.min} readonly={readOnly} onchange={() => editShare(l, k, { species: s.species, form: s.form })} aria-label="Lowest level" />
                <span class="muted">-</span>
                <input class="num" type="number" min="0" max="100" bind:value={s.max} readonly={readOnly} onchange={() => editShare(l, k, { species: s.species, form: s.form })} aria-label="Highest level" />
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
            <button disabled={shares.length < 2} onclick={() => even(l)} title="Share the slots as evenly as they allow">Even split</button>
          </div>
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
  .shares {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .shares li {
    display: grid;
    grid-template-columns: 1rem minmax(0, 1fr) 5.4rem 6rem auto;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
    padding: 1px 2px;
    border-radius: var(--radius-s);
  }
  .shares li.brush {
    background: color-mix(in srgb, var(--gold) 18%, transparent);
  }
  .swatch {
    width: 1rem;
    height: 1rem;
    padding: 0;
    border-radius: 50%;
    border: 2px solid var(--outline);
    box-shadow: none;
    background: hsl(var(--h) 65% 55%);
  }
  .swatch[aria-pressed="true"] {
    outline: 2px solid var(--gold);
    outline-offset: 1px;
  }
  .chance {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.25rem;
    font-variant-numeric: tabular-nums;
  }
  .chance strong {
    min-width: 2.4rem;
    text-align: center;
  }
  .step {
    padding: 0 0.4rem;
    line-height: 1.3;
  }
  .bar {
    display: flex;
    height: 2rem;
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    overflow: hidden;
  }
  .slot {
    min-width: 0;
    padding: 0;
    border: none;
    border-right: 1px solid color-mix(in srgb, var(--outline) 60%, transparent);
    border-radius: 0;
    box-shadow: none;
    background: hsl(var(--h) 65% 55%);
    display: grid;
    place-items: center;
    overflow: hidden;
  }
  .slot:last-child {
    border-right: none;
  }
  .slot.none {
    background: var(--stage);
  }
  .slot:hover:not(:disabled) {
    filter: brightness(1.15);
  }
  .slot img {
    width: 24px;
    height: 24px;
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
