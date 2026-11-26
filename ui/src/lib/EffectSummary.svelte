<script lang="ts">
  // What a move or an item does, in plain words, from its values as they
  // are being edited, and whether it also has battle code
  // (core/src/battle.rs): code runs only for the game's own id, so a copy
  // made with "New move" does only what its data says.
  import { invoke } from "@tauri-apps/api/core";
  import type { TableRecord } from "./tables";

  let {
    table,
    id,
    record,
    names,
    added,
    onOpen,
  }: {
    table: "moves" | "items";
    id: number;
    record: TableRecord | null;
    names: Record<string, string[]>;
    /// Added by the mod (no battle code of its own).
    added: boolean;
    /// Opens another entry of the same table.
    onOpen: (id: number) => void;
  } = $props();

  interface Coded {
    id: number;
    function: string | null;
    label: string | null;
    sharedWith: number[];
  }

  // Conditions by the battle's ids (aletheia ov093/ov095 BTL_SICK_*).
  const CONDITIONS: Record<number, string> = {
    1: "paralyze",
    2: "put to sleep",
    3: "freeze",
    4: "burn",
    5: "poison",
    6: "confuse",
    7: "infatuate",
    8: "trap",
    9: "give a nightmare to",
    10: "curse",
    11: "taunt",
    12: "torment",
    13: "disable a move of",
    14: "make drowsy",
    16: "suppress the ability of",
    17: "identify",
    18: "seed",
    20: "doom (Perish Song)",
    21: "root",
    22: "stop from fleeing",
  };
  const STATS = ["", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed", "accuracy", "evasion"];
  const FLAGS: [string, string][] = [
    ["flagContact", "makes contact"],
    ["flagSound", "sound move"],
    ["flagPunch", "punching move"],
    ["flagCharge", "charges on the first turn"],
    ["flagRecharge", "user must recharge after"],
    ["flagHeal", "healing move"],
    ["flagDefrost", "thaws the user"],
    ["flagIgnoreSubstitute", "goes through Substitute"],
  ];

  let coded = $state<Coded | null>(null);
  $effect(() => {
    const owner = table === "moves" ? "move" : "item";
    const at = id;
    coded = null;
    if (added) return;
    invoke<Coded | null>("battle_coded", { owner, id: at })
      .then((c) => {
        if (id === at) coded = c;
      })
      .catch(() => {});
  });

  const v = $derived.by(() => {
    const out: Record<string, number> = {};
    for (const s of record?.sections ?? []) if (s.kind === "fields") for (const x of s.values) out[x.key] = Number(x.value);
    return out;
  });
  const sign = (n: number) => (n > 0 ? `+${n}` : `${n}`);
  const chance = (p: number) => (p > 0 && p < 100 ? `${p}% chance: ` : "");
  const nameOf = (en: string, i: number) => names[en]?.[i] ?? String(i);

  // Rules the battle code needs, or that every one of the game's moves
  // follows (E:/dev/research/kenon-inferred/move_rules.py). `hard` breaks the
  // move in battle; the others are what the game always does.
  interface Check {
    text: string;
    hard: boolean;
    fix?: { label: string; set: Record<string, number> };
  }
  const DAMAGING = [0, 4, 6, 7, 8, 9];
  const STATUS_KINDS = [1, 2, 3, 5, 10, 11, 12];
  function setValue(key: string, value: number) {
    for (const s of record?.sections ?? [])
      if (s.kind === "fields") {
        const f = s.values.find((x) => x.key === key);
        if (f) f.value = value;
      }
  }
  const checks = $derived.by(() => {
    const out: Check[] = [];
    if (table !== "moves" || !record) return out;
    const cat = v.effectCategory;
    const kind = names.moveCategory?.[cat] ?? `kind ${cat}`;
    const hasStat = [1, 2, 3].some((k) => v[`rankType${k}`] && v[`rankValue${k}`]);
    if (v.sickId && v.sickId !== 0xffff && !v.sickContType)
      out.push({ text: "It causes a condition but the condition has no duration, so the game never applies it.", hard: true, fix: { label: "Lasts until cured", set: { sickContType: 1 } } });
    if (!v.sickId && v.sickContType)
      out.push({ text: "It has a condition duration but no condition.", hard: true, fix: { label: "Clear the duration", set: { sickContType: 0 } } });
    if ([1, 4, 5].includes(cat) && !v.sickId)
      out.push({ text: `"${kind}" needs a condition: the battle applies the move's condition and there is none.`, hard: true });
    if ([2, 5, 6, 7].includes(cat) && !hasStat)
      out.push({ text: `"${kind}" needs a stat change: there is none to apply.`, hard: true });
    if (cat === 6 && [1, 2, 3].some((k) => v[`rankType${k}`] && v[`rankValue${k}`] > 0))
      out.push({ text: `"${kind}" lowers stats; a raising change here is unlike every game move.`, hard: false });
    if (cat === 8 && v.drainRatio <= 0)
      out.push({ text: `"${kind}" needs a drain above 0 to restore any HP.`, hard: true, fix: { label: "Drain 50%", set: { drainRatio: 50 } } });
    if (cat === 3 && v.healRatio <= 0)
      out.push({ text: `"${kind}" needs a heal above 0.`, hard: true, fix: { label: "Heal 50%", set: { healRatio: 50 } } });
    if (cat === 10 && v.target !== 10)
      out.push({ text: `"${kind}" works on the whole field; every game move of this kind targets the whole field.`, hard: false, fix: { label: "Target the whole field", set: { target: 10 } } });
    if (cat !== 10 && v.target === 10)
      out.push({ text: "It targets the whole field, which only field-changing moves do.", hard: false, fix: { label: "Kind: changes the whole field", set: { effectCategory: 10 } } });
    if (DAMAGING.includes(cat) && cat !== 9 && v.power === 0)
      out.push({ text: `"${kind}" deals damage but its power is 0.`, hard: true });
    // Me First is kind 0 and Status: its own battle code takes over.
    if (DAMAGING.includes(cat) && v.damageClass === 0 && !coded)
      out.push({ text: `"${kind}" deals damage but it is a Status move; make it Physical or Special.`, hard: true });
    if (STATUS_KINDS.includes(cat) && v.damageClass !== 0)
      out.push({ text: `"${kind}" deals no damage; every such game move is a Status move.`, hard: false, fix: { label: "Make it Status", set: { damageClass: 0, power: 0 } } });
    if ((v.flagMagicCoat || v.flagSnatch) && v.damageClass !== 0)
      out.push({ text: "Magic Coat and Snatch only ever apply to status moves in the game.", hard: false });
    if (v.flagPunch && !v.flagContact)
      out.push({ text: "Every punching move in the game makes contact.", hard: false, fix: { label: "Makes contact", set: { flagContact: 1 } } });
    if (v.drainRatio && !DAMAGING.includes(cat) && cat !== 13)
      out.push({ text: "Drain and recoil only work on moves that deal damage.", hard: false });
    return out;
  });

  const lines = $derived.by(() => {
    const out: string[] = [];
    if (table === "moves") {
      const cls = ["Status", "Physical", "Special"][v.damageClass] ?? "";
      const acc = v.accuracy >= 101 || v.accuracy === 0 ? "never misses" : `${v.accuracy}% accuracy`;
      out.push(
        `${nameOf("type", v.type)} ${cls.toLowerCase()} move` +
          (v.power > 1 ? `, power ${v.power}` : v.power === 1 ? ", power set by its effect" : "") +
          `, ${acc}, ${v.pp} PP.`,
      );
      if (v.priority) out.push(v.priority > 0 ? `Goes first (priority ${sign(v.priority)}).` : `Goes last (priority ${v.priority}).`);
      if (v.hitMax > 1) out.push(v.hitMin === v.hitMax ? `Hits ${v.hitMax} times.` : `Hits ${v.hitMin} to ${v.hitMax} times.`);
      if (v.sickId === 0xffff) out.push("Causes a condition its own battle code picks (Tri Attack, Smack Down).");
      else if (v.sickId) {
        const what = CONDITIONS[v.sickId] ?? `give condition ${v.sickId} to`;
        out.push(`${chance(v.sickPercent)}${what[0].toUpperCase()}${what.slice(1)} the target.`);
      }
      for (const k of [1, 2, 3]) {
        const stat = v[`rankType${k}`];
        const by = v[`rankValue${k}`];
        if (stat && by) out.push(`${chance(v[`rankPercent${k}`])}${STATS[stat] ?? `stat ${stat}`} ${sign(by)}.`);
      }
      if (v.shrinkPercent) out.push(`${v.shrinkPercent}% chance to make the target flinch.`);
      if (v.drainRatio > 0) out.push(`The user gets back ${v.drainRatio}% of the damage dealt.`);
      if (v.drainRatio < 0) out.push(`The user takes ${-v.drainRatio}% of the damage dealt as recoil.`);
      if (v.healRatio > 0) out.push(`Heals the user by ${v.healRatio}% of its HP.`);
      if (v.healRatio < 0) out.push(`Costs the user ${-v.healRatio}% of its HP.`);
      if (v.criticalRank && v.criticalRank !== 6) out.push(`Higher chance of a critical hit (${sign(v.criticalRank)}).`);
      if (v.criticalRank === 6) out.push("Always a critical hit.");
      const flags = FLAGS.filter(([k]) => v[k]).map(([, t]) => t);
      if (!v.flagProtect) flags.push("not stopped by Protect");
      if (flags.length) out.push(`${flags[0][0].toUpperCase()}${flags.join(", ").slice(1)}.`);
    } else {
      if (v.extType === 1) {
        if (v.hpRestore) out.push(v.hpRestoreAmount >= 255 ? "Restores all HP." : `Restores ${v.hpRestoreAmount} HP.`);
        const cures = [
          ["cureSleep", "sleep"],
          ["curePoison", "poison"],
          ["cureBurn", "burn"],
          ["cureFreeze", "freeze"],
          ["cureParalysis", "paralysis"],
          ["cureConfusion", "confusion"],
          ["cureInfatuation", "infatuation"],
        ]
          .filter(([k]) => v[k])
          .map(([, t]) => t);
        if (cures.length === 7) out.push("Cures every status problem.");
        else if (cures.length) out.push(`Cures ${cures.join(", ")}.`);
        if (v.revive) out.push(v.reviveAll ? "Revives every fainted Pokemon." : "Revives a fainted Pokemon.");
        if (v.levelUp) out.push("Raises the level by one.");
        if (v.evolve) out.push("Makes a Pokemon evolve.");
        if (v.ppRestore) out.push(v.ppRestoreAmount >= 127 ? "Restores all PP." : `Restores ${v.ppRestoreAmount} PP.`);
        if (v.ppUp) out.push(v.ppUpMax ? "Raises a move's PP to the most." : "Raises a move's PP.");
        const stages = [
          ["atkStage", "Attack"],
          ["defStage", "Defense"],
          ["spAtkStage", "Sp. Atk"],
          ["spDefStage", "Sp. Def"],
          ["speedStage", "Speed"],
          ["accStage", "accuracy"],
          ["critStage", "critical-hit ratio"],
        ]
          .filter(([k]) => v[k])
          .map(([, t]) => t);
        if (stages.length) out.push(`In battle, raises ${stages.join(", ")}.`);
        const evs = [
          ["hpEv", "HP"],
          ["atkEv", "Attack"],
          ["defEv", "Defense"],
          ["speedEv", "Speed"],
          ["spAtkEv", "Sp. Atk"],
          ["spDefEv", "Sp. Def"],
        ]
          .filter(([k]) => v[k])
          .map(([k, t]) => `${t} ${sign(v[k])}`);
        if (evs.length) out.push(`Effort values: ${evs.join(", ")}.`);
      }
      if (v.price) out.push(`Costs ${v.price * 10} in shops, sells for ${v.price * 5}.`);
      if (v.flingPower) out.push(`Fling power ${v.flingPower}.`);
      if (!out.length) out.push("No effect of its own in its data.");
    }
    return out;
  });
</script>

{#if checks.length}
  <div class="window block checks">
    <h3>Checks</h3>
    <ul>
      {#each checks as c}
        <li class:hard={c.hard}>
          <span>{c.hard ? "Will not work:" : "Unusual:"} {c.text}</span>
          {#if c.fix}
            <button class="quiet" onclick={() => Object.entries(c.fix!.set).forEach(([k, x]) => setValue(k, x))}>{c.fix.label}</button>
          {/if}
        </li>
      {/each}
    </ul>
  </div>
{/if}

<div class="window block effect">
  <h3>What it does</h3>
  <ul>
    {#each lines as l}<li>{l}</li>{/each}
  </ul>
  {#if added}
    <p class="note">
      Added by this mod. It does what the values above say; to run another {table === "moves" ? "move" : "item"}'s battle
      code too, pick it under "In battle".
    </p>
  {:else if coded}
    <p class="note">
      It also has battle code of its own: <strong>{coded.label ?? coded.function}</strong>{#if coded.function}<span class="muted"> ({coded.function})</span>{/if}.
      A copy gets it by picking this {table === "moves" ? "move" : "item"} under "In battle".
      {#if coded.sharedWith.length}
        The same code runs for
        {#each coded.sharedWith as other, i}{i ? ", " : ""}<button class="link" onclick={() => onOpen(other)}>{nameOf(table === "moves" ? "move" : "item", other)}</button>{/each}.
      {/if}
    </p>
  {:else}
    <p class="note muted">Everything it does is in its values, so a copy works the same.</p>
  {/if}
</div>

<style>
  /* Its own padding: the table editor's .block style does not reach a child component. */
  .block {
    padding: 0.9rem 1rem;
  }
  h3 {
    margin: 0 0 0.4rem;
  }
  .checks ul {
    margin: 0.35rem 0 0;
    padding-left: 1.2rem;
  }
  .checks li {
    margin: 0.25rem 0;
    color: var(--gold);
  }
  .checks li.hard {
    color: var(--bad);
  }
  .checks li button {
    margin-left: 0.5rem;
    padding: 0.1rem 0.6rem;
    font-size: 0.85rem;
  }
  .effect ul {
    margin: 0.35rem 0 0.5rem;
    padding-left: 1.2rem;
  }
  .effect li {
    margin: 0.1rem 0;
  }
  .note {
    font-size: 0.9rem;
  }
  .link {
    background: none;
    border: none;
    box-shadow: none;
    padding: 0;
    color: var(--accent);
    font-weight: 600;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
