// Wild encounter slots. The game picks a slot with fixed odds per method
// (aletheia ov021/field_encount.c FIELD_ENCOUNT_PickGroundSlot,
// PickWaterSlot, PickFishingSlot); a map only chooses which Pokemon sits
// in each slot. A Pokemon's chance is the sum of its slots' odds.

const LAND = [20, 20, 10, 10, 10, 10, 5, 5, 4, 4, 1, 1];
const WATER = [60, 30, 5, 4, 1];
const FISHING = [40, 40, 15, 4, 1];

/// The odds of each slot of a method (by its list key and slot count).
export function slotOdds(key: string, count: number): number[] {
  if (count === 12) return LAND;
  if (key.startsWith("fishing")) return FISHING;
  return WATER;
}

/// Gives each slot to one of the wanted Pokemon so their summed odds come
/// as close to the wanted percentages as the fixed slots allow. Returns,
/// per slot, the index into `wanted`.
export function assignSlots(odds: number[], wanted: number[]): number[] {
  const n = wanted.length;
  if (!n) return odds.map(() => 0);
  const got = new Array(n).fill(0);
  const owner = new Array(odds.length).fill(0);
  // Largest slots first, each to whoever is furthest below their share.
  const order = odds.map((_, i) => i).sort((a, b) => odds[b] - odds[a]);
  for (const s of order) {
    let best = 0;
    for (let k = 1; k < n; k++) if (wanted[k] - got[k] > wanted[best] - got[best]) best = k;
    owner[s] = best;
    got[best] += odds[s];
  }
  const error = () => got.reduce((e, g, k) => e + Math.abs(g - wanted[k]), 0);
  // Then move or swap single slots while that brings the split closer.
  for (let pass = 0; pass < 40; pass++) {
    let better = false;
    for (let s = 0; s < odds.length; s++) {
      for (let k = 0; k < n; k++) {
        const from = owner[s];
        if (k === from) continue;
        const before = error();
        got[from] -= odds[s];
        got[k] += odds[s];
        if (error() < before - 1e-9) {
          owner[s] = k;
          better = true;
        } else {
          got[from] += odds[s];
          got[k] -= odds[s];
        }
      }
      for (let t = s + 1; t < odds.length; t++) {
        const [a, b] = [owner[s], owner[t]];
        if (a === b || odds[s] === odds[t]) continue;
        const before = error();
        got[a] += odds[t] - odds[s];
        got[b] += odds[s] - odds[t];
        if (error() < before - 1e-9) {
          [owner[s], owner[t]] = [b, a];
          better = true;
        } else {
          got[a] -= odds[t] - odds[s];
          got[b] -= odds[s] - odds[t];
        }
      }
    }
    if (!better) break;
  }
  // Everyone wanted gets at least the smallest free slot when one can be
  // taken from someone holding several.
  for (let k = 0; k < n; k++) {
    if (wanted[k] <= 0 || owner.includes(k)) continue;
    const counts = new Array(n).fill(0);
    owner.forEach((o) => counts[o]++);
    const spare = order
      .slice()
      .reverse()
      .find((s) => counts[owner[s]] > 1);
    if (spare !== undefined) owner[spare] = k;
  }
  return owner;
}

/// What each wanted Pokemon actually gets with a slot assignment.
export function achieved(odds: number[], owner: number[], count: number): number[] {
  const got = new Array(count).fill(0);
  owner.forEach((k, s) => (got[k] += odds[s]));
  return got;
}
