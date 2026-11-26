<script lang="ts">
  // An experience curve (a/0/1/7: total experience for levels 1 to 100) as
  // a graph. Drag a point up or down (it stays between its neighbours, so
  // the curve keeps rising), type a value, or change a selection at once:
  // scale it, or set it to one of the game's growth formulas.
  import type { Value } from "./tables";

  let {
    values,
    readOnly,
    rate,
  }: {
    /// The section's values: level0 (fixed at 0) and level#0..level#99.
    values: Value[];
    readOnly: boolean;
    /// The growth rate's name, for the heading.
    rate: string | null;
  } = $props();

  // Level n (1..100) is values key level#(n-1).
  const points = $derived(
    Array.from({ length: 100 }, (_, i) => values.find((v) => v.key === `level#${i}`)).filter((v): v is Value => !!v),
  );
  const exp = (i: number) => Number(points[i]?.value ?? 0);

  // Geometry.
  const W = 900;
  const H = 300;
  const PAD = { l: 64, r: 12, t: 12, b: 28 };
  let top = $state(0);
  $effect(() => {
    // The scale follows the curve, with room to drag up; fixed while dragging.
    if (drag === null) top = niceMax(Math.max(1, ...points.map((_, i) => exp(i))) * 1.08);
  });
  function niceMax(v: number) {
    const p = 10 ** Math.floor(Math.log10(v));
    return Math.ceil(v / p) * p;
  }
  const x = (i: number) => PAD.l + (i / 99) * (W - PAD.l - PAD.r);
  const y = (v: number) => PAD.t + (1 - v / top) * (H - PAD.t - PAD.b);
  const path = $derived(points.map((_, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(exp(i)).toFixed(1)}`).join(""));
  const ticks = $derived([0, 0.25, 0.5, 0.75, 1].map((f) => Math.round(top * f)));

  // The range a point may take: above the level before, below the level after.
  function bounds(i: number): [number, number] {
    const lo = i === 0 ? 0 : exp(i - 1) + 1;
    const hi = i === 99 ? 0xffffff : exp(i + 1) - 1;
    return [lo, Math.max(lo, hi)];
  }
  function set(i: number, v: number) {
    const [lo, hi] = bounds(i);
    points[i].value = Math.round(Math.min(hi, Math.max(lo, v)));
  }

  // Selection: click a point; shift-click for a range; ctrl-click to add.
  let selected = $state<Set<number>>(new Set());
  let anchor = $state<number | null>(null);
  let focus = $state<number | null>(0);
  function pick(i: number, e: MouseEvent) {
    const next = new Set(e.ctrlKey || e.metaKey ? selected : []);
    if (e.shiftKey && anchor !== null) {
      for (let k = Math.min(anchor, i); k <= Math.max(anchor, i); k++) next.add(k);
    } else if ((e.ctrlKey || e.metaKey) && next.has(i)) {
      next.delete(i);
    } else {
      next.add(i);
      anchor = i;
    }
    selected = next;
    focus = i;
  }

  // Dragging a point.
  let svg = $state<SVGSVGElement | null>(null);
  let drag = $state<number | null>(null);
  function toValue(clientY: number) {
    if (!svg) return 0;
    const r = svg.getBoundingClientRect();
    const sy = ((clientY - r.top) / r.height) * H;
    return ((H - PAD.b - sy) / (H - PAD.t - PAD.b)) * top;
  }
  function down(i: number, e: PointerEvent) {
    pick(i, e);
    if (readOnly) return;
    drag = i;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (drag !== null) set(drag, toValue(e.clientY));
  }
  function up() {
    drag = null;
  }

  // The game's growth formulas (total experience at level n).
  const FORMULAS: Record<string, (n: number) => number> = {
    "Medium Fast": (n) => n ** 3,
    Erratic: (n) =>
      n <= 50
        ? (n ** 3 * (100 - n)) / 50
        : n <= 68
          ? (n ** 3 * (150 - n)) / 100
          : n <= 98
            ? (n ** 3 * Math.floor((1911 - 10 * n) / 3)) / 500
            : (n ** 3 * (160 - n)) / 100,
    Fluctuating: (n) =>
      n <= 15
        ? (n ** 3 * (Math.floor((n + 1) / 3) + 24)) / 50
        : n <= 36
          ? (n ** 3 * (n + 14)) / 50
          : (n ** 3 * (Math.floor(n / 2) + 32)) / 50,
    "Medium Slow": (n) => (6 / 5) * n ** 3 - 15 * n ** 2 + 100 * n - 140,
    Fast: (n) => (4 * n ** 3) / 5,
    Slow: (n) => (5 * n ** 3) / 4,
  };
  // The formula starts as the curve's own growth rate.
  let formula = $state("Medium Fast");
  $effect(() => {
    if (rate && rate in FORMULAS) formula = rate;
  });
  let scale = $state(100);

  const targets = $derived(selected.size ? [...selected].sort((a, b) => a - b) : points.map((_, i) => i));
  function applyScale() {
    const f = scale / 100;
    const next = points.map((_, i) => exp(i));
    for (const i of targets) next[i] = Math.round(next[i] * f);
    write(next);
  }
  function applyFormula() {
    const next = points.map((_, i) => exp(i));
    for (const i of targets) next[i] = Math.max(0, Math.floor(FORMULAS[formula](i + 1)));
    next[0] = 0;
    write(next);
  }
  // Writes a whole curve, keeping it rising (each level above the last).
  function write(next: number[]) {
    for (let i = 1; i < next.length; i++) if (next[i] <= next[i - 1]) next[i] = next[i - 1] + 1;
    next.forEach((v, i) => (points[i].value = v));
  }

  const fmt = (n: number) => n.toLocaleString("en-US");
</script>

<div class="curve">
  <div class="head">
    <h4>{rate ? `${rate} curve` : "Experience curve"}</h4>
    <span class="muted">Shared by all Pokemon with this growth rate. Level 100: {fmt(exp(99))} EXP.</span>
  </div>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <svg bind:this={svg} viewBox="0 0 {W} {H}" class:dragging={drag !== null} onpointermove={move} onpointerup={up} onpointercancel={up}>
    {#each ticks as t}
      <line class="grid" x1={PAD.l} x2={W - PAD.r} y1={y(t)} y2={y(t)} />
      <text class="axis" x={PAD.l - 6} y={y(t) + 4} text-anchor="end">{t >= 1e6 ? `${(t / 1e6).toFixed(t % 1e6 ? 2 : 0)}M` : t >= 1e3 ? `${Math.round(t / 1e3)}k` : t}</text>
    {/each}
    {#each [1, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100] as lv}
      <text class="axis" x={x(lv - 1)} y={H - 8} text-anchor="middle">{lv}</text>
    {/each}
    <path class="line" d={path} />
    {#each points as _, i}
      <circle
        class="node"
        class:sel={selected.has(i)}
        class:focus={focus === i}
        cx={x(i)}
        cy={y(exp(i))}
        r={selected.has(i) || focus === i ? 5 : 3.2}
        role="button"
        tabindex="-1"
        aria-label="Level {i + 1}: {exp(i)}"
        onpointerdown={(e) => down(i, e)}
      >
        <title>Level {i + 1}: {fmt(exp(i))}</title>
      </circle>
    {/each}
  </svg>

  <div class="tools">
    {#if focus !== null && points[focus]}
      {@const [lo, hi] = bounds(focus)}
      <label class="field">
        <span class="name">Level {focus + 1}</span>
        <input
          type="number"
          min={lo}
          max={hi}
          value={exp(focus)}
          readonly={readOnly}
          onchange={(e) => set(focus!, Number(e.currentTarget.value))}
        />
      </label>
      <span class="muted range">{fmt(lo)} to {fmt(hi)}</span>
    {/if}
    {#if !readOnly}
      <span class="sep"></span>
      <span class="muted">Applies to {selected.size ? `${selected.size} selected level${selected.size > 1 ? "s" : ""}` : "all levels"}</span>
      <label class="field small">
        <span class="name">Scale (%)</span>
        <input type="number" min="1" max="1000" bind:value={scale} />
      </label>
      <button onclick={applyScale}>Scale</button>
      <label class="field">
        <span class="name">Growth formula</span>
        <select bind:value={formula}>
          {#each Object.keys(FORMULAS) as f}<option value={f}>{f}</option>{/each}
        </select>
      </label>
      <button onclick={applyFormula}>Apply formula</button>
      {#if selected.size}<button class="quiet" onclick={() => (selected = new Set())}>Clear selection</button>{/if}
    {/if}
  </div>
  <p class="muted hint">Drag points to adjust; Shift-click selects a range, Ctrl-click adds a point.</p>
</div>

<style>
  .curve {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 0.8rem;
    flex-wrap: wrap;
  }
  .head h4 {
    margin: 0;
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .head .muted {
    font-size: 0.85rem;
  }
  svg {
    width: 100%;
    max-width: 70rem;
    height: auto;
    background: var(--bg);
    border: 2px solid var(--outline);
    border-radius: var(--radius-s);
    touch-action: none;
    user-select: none;
  }
  .grid {
    stroke: var(--rim);
    stroke-width: 1;
  }
  .axis {
    fill: var(--muted);
    font-size: 11px;
  }
  .line {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
  }
  .node {
    fill: var(--panel-2);
    stroke: var(--accent);
    stroke-width: 1.5;
    cursor: ns-resize;
  }
  .node.sel {
    fill: var(--accent);
  }
  .node.focus {
    stroke: var(--gold);
    stroke-width: 2.5;
  }
  .dragging {
    cursor: ns-resize;
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 0.6rem 0.9rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    width: 9rem;
  }
  .field.small {
    width: 6rem;
  }
  .field .name {
    font-size: 0.85rem;
    color: var(--muted);
  }
  .range {
    font-size: 0.8rem;
    padding-bottom: 0.5rem;
  }
  .sep {
    width: 1px;
    align-self: stretch;
    background: var(--rim);
  }
  .tools > .muted:not(.range) {
    padding-bottom: 0.5rem;
    font-size: 0.85rem;
  }
  .hint {
    margin: 0;
    font-size: 0.8rem;
  }
</style>
