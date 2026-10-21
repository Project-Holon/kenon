// Plain test and timing for search.ts. Run: node ui/src/lib/moddocs/search.test.mjs
import { readFileSync } from "node:fs";
import { buildEntries } from "./index.ts";
import { buildIndex, searchIndex } from "./search.ts";

const api = JSON.parse(readFileSync(new URL("./api.json", import.meta.url), "utf8"));
const entries = buildEntries(api);

let t = performance.now();
const idx = buildIndex(api, entries);
const build = performance.now() - t;

// Each case: the query and the ids of which one must be in the top 3.
const cases = [
  ["release fainted pokemon", ["recipe:release-on-faint", "event:mon.release", "event:battle.faint"]],
  ["pokemon dies", ["event:battle.faint"]],
  ["level cap badge", ["recipe:level-cap", "event:badge.get"]],
  ["give item", ["event:item.add", "fn:item_add"]],
  ["heal", ["event:heal", "event:heal.hp"]],
  ["shiny", ["fn:species_shiny", "recipe:fished-shiny"]],
  ["faint", ["event:battle.faint"]],
  ["fainetd", ["event:battle.faint"]],
  ["mon_rele", ["event:mon.release"]],
  ["money", ["event:money.change", "fn:money"]],
];

let bad = 0;
for (const [q, want] of cases) {
  const hits = searchIndex(idx, q);
  const top = hits.slice(0, 3).map((h) => h.id);
  const ok = want.some((w) => top.includes(w));
  if (!ok) bad++;
  console.log(`${ok ? "ok  " : "FAIL"} ${JSON.stringify(q)} -> ${top.join(", ")}`);
}

const qs = cases.map((c) => c[0]);
t = performance.now();
const rounds = 200;
for (let i = 0; i < rounds; i++) for (const q of qs) searchIndex(idx, q);
const query = (performance.now() - t) / (rounds * qs.length);
console.log(`entries ${entries.length}, first build ${build.toFixed(1)} ms, query ${query.toFixed(2)} ms`);
if (bad) process.exit(1);
