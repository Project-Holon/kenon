// Builds ui/src/lib/moddocs/api.json from eidos's published API description
// (sdk/api/v4) plus kenon's own text (ui/src/lib/moddocs/curated.mjs).
//
//   node tools/mod-docs.mjs [--eidos <folder>]
//
// The eidos folder is --eidos, then $EIDOS, then ../eidos. Point it at a
// checkout of the commit you want (git worktree add <dir> <commit>).
//
// The structure comes from sdk/api/v4: events, payload layouts, rule state,
// services, state calls, constants, hooks, and eidos_wasm.h for the C
// signatures and the field comments. The v4 files carry no prose, so the
// words come from two places: eidos's docs (docs/mod-events.md for the game
// function and the refusal outcome of each event, docs/mod-payloads.md for
// the meaning of each field and the per-source outcomes, docs/mod-rules-
// language.md for the state fields) and curated.mjs. Both are checked against
// v4, and a mismatch is printed as a warning. Hook prototypes are not in v4:
// they are kept from the last api.json, or read from a build when there is one.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const args = process.argv.slice(2);
const flag = args.indexOf("--eidos");
const eidos = resolve(flag >= 0 ? args[flag + 1] : (process.env.EIDOS ?? join(root, "..", "eidos")));
const outDir = join(root, "ui", "src", "lib", "moddocs");
const warnings = [];
const warn = (m) => warnings.push(m);

const read = (rel) => {
  const p = join(eidos, rel);
  if (!existsSync(p)) throw new Error(`missing ${p}`);
  return readFileSync(p, "utf8").replace(/\r\n/g, "\n");
};
const v4 = (name) => JSON.parse(read(`sdk/api/v4/${name}`));

// C spells the constants EIDOS_ in the wasm header; the JSON and the eidos
// docs spell them BW_. The docs show what a code mod types.
const eidosName = (s) => s.replace(/\bBW_(?=[A-Z])/g, "EIDOS_");
const docType = (t) => t.replace(/\bBwMonRef\b/g, "EidosMonRef");
// The type as eidos_wasm.h spells it.
const cType = (t) =>
  docType(t)
    .replace(/\bu(8|16|32|64)\b/g, "uint$1_t")
    .replace(/\bs(8|16|32|64)\b/g, "int$1_t");
// "u32 *" and "out" make "u32 *out"; "u32" and "field" make "u32 field".
const decl = (t, n) => (t.endsWith("*") ? `${t}${n}` : `${t} ${n}`);

// ---------------------------------------------------------------- markdown

const cells = (line) =>
  line
    .trim()
    .replace(/^\||\|$/g, "")
    .split("|")
    .map((c) => c.trim());
const isRow = (l) => l.trimStart().startsWith("|");
const isRule = (l) => /^\s*\|[\s:|-]+\|\s*$/.test(l);
const ticks = (s) => [...s.matchAll(/`([^`]+)`/g)].map((m) => m[1]);
const plain = (s) =>
  eidosName(
    s
      .replace(/`/g, "")
      .replace(/\*\*/g, "")
      .replace(/\*([^*]+)\*/g, "$1")
      .trim(),
  );

// Bullets of a block: "- text" with indented continuation lines.
function bullets(lines) {
  const out = [];
  for (const l of lines) {
    if (l.startsWith("- ")) out.push(l.slice(2).trim());
    else if (out.length && /^\s{2,}\S/.test(l)) out[out.length - 1] += " " + l.trim();
  }
  return out;
}

// ---------------------------------------------------------------- v4 files

const eventsJson = v4("events.json");
const rulesJson = v4("rules.json");
const stateJson = v4("state.json");
const servicesJson = v4("services.json");
const enumsJson = v4("enums.json");
const hooksJson = v4("hooks.json");
const header = read("sdk/api/v4/eidos_wasm.h");
const apiVersion = eventsJson.api;
for (const [n, j] of [["rules", rulesJson], ["state", stateJson], ["services", servicesJson], ["enums", enumsJson], ["hooks", hooksJson]]) {
  if (j.api !== apiVersion) warn(`${n}.json is api ${j.api}, events.json is api ${apiVersion}`);
}

const curated = (await import(pathToFileURL(join(outDir, "curated.mjs")).href)).default;

// Field comments of the header's payload structs: the enum a field takes.
const headerFields = new Map();
for (const m of header.matchAll(/typedef struct (Eidos\w+) \{([\s\S]*?)\} \1;/g)) {
  const fields = new Map();
  for (const line of m[2].split("\n")) {
    const f = line.match(/^\s*(?:const )?[\w\s*]+?\s+\*?(\w+)(?:\[\d+\])?;\s*(?:\/\*\s*(.*?)\s*\*\/)?/);
    if (f) fields.set(f[1], f[2] ?? "");
  }
  headerFields.set(m[1], fields);
}

// ---------------------------------------------------------------- events

const ruleEventNames = new Set(rulesJson.events);
const events = new Map();
for (const e of eventsJson.events) {
  const cStruct = e.struct.replace(/^Bw/, "Eidos");
  const hf = headerFields.get(cStruct);
  if (!hf) warn(`no struct ${cStruct} in eidos_wasm.h for ${e.name}`);
  const payload = e.fields.map((f) => {
    const name = f.name.replace(/_$/, "");
    const comment = hf?.get(f.name) ?? "";
    return {
      name,
      type: docType(f.type) + (f.count !== null ? `[${f.count}]` : ""),
      meaning: "",
      writable: f.writable,
      cName: f.name,
      cType: cType(f.type) + (f.count !== null ? `[${f.count}]` : ""),
      ruleKind: f.count !== null ? "list" : f.type === "BwMonRef" ? "mon" : "int",
      comment,
    };
  });
  events.set(e.name, {
    name: e.name,
    category: null,
    binding: "",
    region: "",
    refuse: null,
    modifyText: null,
    notes: [],
    caveats: [],
    outcomes: [],
    payload,
    cStruct,
    numId: e.id,
    refusable: e.refusable,
    ruleEvent: ruleEventNames.has(e.name),
  });
}
for (const n of ruleEventNames) if (!events.has(n)) warn(`rules.json names ${n}, events.json has no such event`);

const evDoc = read("docs/mod-events.md").split("\n");
const sec1 = evDoc.slice(
  evDoc.findIndex((l) => l.startsWith("## 1.")),
  evDoc.findIndex((l) => l.startsWith("## 2.")),
);
const CATEGORY_NAMES = {
  Session: "Session",
  World: "World",
  Encounters: "Encounters",
  Pokemon: "Pokemon",
  Battle: "Battle",
  "Items, money and healing": "Items and money",
  Progress: "Progress",
};
const categories = [];
{
  let cat = null;
  let block = [];
  const flush = () => {
    if (cat) {
      for (const b of bullets(block)) {
        const m = b.match(/^\*\*`([a-z.]+)`\*\*/);
        if (m && events.has(m[1])) events.get(m[1]).notes.push(plain(b.replace(/^\*\*`[a-z.]+`\*\*[:,]?\s*/, "")));
      }
    }
    block = [];
  };
  for (const l of sec1) {
    const h = l.match(/^### (.+)/);
    if (h) {
      flush();
      cat = CATEGORY_NAMES[h[1].trim()] ?? null;
      if (cat) categories.push(cat);
      continue;
    }
    if (!cat) continue;
    block.push(l);
    if (isRow(l) && !isRule(l)) {
      const c = cells(l);
      const names = ticks(c[0]);
      if (c[0] === "Event" || names.length !== 1 || c.length < 5) continue;
      const ev = events.get(names[0]);
      if (!ev) {
        warn(`docs/mod-events.md lists ${names[0]}, sdk/api/v4 does not`);
        continue;
      }
      ev.category = cat;
      ev.binding = plain(c[1]);
      ev.region = plain(c[2]);
      ev.refuse = c[3] === "-" ? null : plain(c[3]);
      ev.modifyText = c[4] === "-" ? null : plain(c[4]);
    }
  }
  flush();
}
for (const ev of events.values()) {
  if (ev.category === null && !curated.events[ev.name]?.category) warn(`${ev.name} is not in docs/mod-events.md`);
  if ((ev.refuse !== null) !== ev.refusable && ev.category !== null) {
    warn(`${ev.name}: docs/mod-events.md and events.json disagree on refusing`);
  }
}

// What the builders fill: caveats per event.
{
  const i = evDoc.findIndex((l) => l.startsWith("### What the builders fill"));
  const j = evDoc.findIndex((l, k) => k > i && l.startsWith("### The contexts"));
  for (const b of bullets(evDoc.slice(i, j))) {
    const m = b.match(/^((?:`[a-z.]+`(?:'s)?(?:,| and)? ?)+)/);
    if (!m) continue;
    for (const n of ticks(m[1])) if (events.has(n)) events.get(n).caveats.push(plain(b));
  }
}

// Corpus names of renamed hook rows.
const renamed = new Map();
for (const l of evDoc) {
  if (!isRow(l) || isRule(l)) continue;
  const c = cells(l);
  if (c.length === 4 && /^0x[0-9A-Fa-f]{8}$/.test(c[0]) && c[2].startsWith("`") && c[3].startsWith("`")) {
    renamed.set(`${c[1]}:${parseInt(c[0], 16)}`, ticks(c[3])[0]);
  }
}

// Field meanings and the per-source outcomes, from docs/mod-payloads.md.
{
  const lines = read("docs/mod-payloads.md").split("\n");
  const start = lines.findIndex((l) => l.startsWith("## 2."));
  let names = null;
  let seenRow = false;
  let inOutcomes = false;
  let block = [];
  const flushOutcomes = () => {
    if (names) {
      for (const b of bullets(block)) {
        const m = b.match(/^\*\*(.+?)\*\*:\s*(.*)$/);
        const text = plain(m ? `${m[1]}: ${m[2]}` : b);
        for (const n of names) events.get(n).outcomes.push(text);
      }
    }
    block = [];
    inOutcomes = false;
  };
  for (let i = start + 1; i < lines.length; i++) {
    const l = lines[i];
    if (l.startsWith("## ")) break;
    if (l.startsWith("**`")) {
      flushOutcomes();
      names = ticks(l.split(":")[0]).filter((n) => events.has(n));
      if (!names.length) names = null;
      seenRow = false;
      continue;
    }
    if (names && /^Where it fires and what a refusal does/.test(l)) {
      inOutcomes = true;
      continue;
    }
    if (inOutcomes) {
      if (l.trim() === "" && block.length && !/^- /.test(lines[i + 1] ?? "") && !/^\s{2,}\S/.test(lines[i + 1] ?? "")) {
        flushOutcomes();
        continue;
      }
      block.push(l);
      continue;
    }
    if (names && isRow(l) && !isRule(l)) {
      const c = cells(l);
      if (c[0] === "Field") continue;
      seenRow = true;
      let fieldNames = ticks(c[0]);
      // A row that names a range, `move1` .. `move4`, covers the names between.
      const range = c[0].match(/`(\w*?)(\d+)` \.\. `\1(\d+)`/);
      if (range) {
        fieldNames = [];
        for (let k = Number(range[2]); k <= Number(range[3]); k++) fieldNames.push(range[1] + k);
      }
      const writable = /\*mod\*/.test(c[1]);
      const only = c[2].match(/^`?([a-z]+.[a-z]+)`? only/);
      for (const n of names) {
        if (only && only[1] !== n) continue;
        for (const f of fieldNames) {
          const pf = events.get(n).payload.find((p) => p.name === f || p.cName === f);
          if (!pf) {
            warn(`docs/mod-payloads.md: ${n}.${f} is not in sdk/api/v4`);
            continue;
          }
          pf.meaning = plain(c[2]);
          if (pf.writable !== writable) warn(`${n}.${f}: docs/mod-payloads.md and events.json disagree on *mod*`);
        }
      }
    } else if (names && seenRow && l.trim() && !isRow(l)) {
      names = null;
    }
  }
  flushOutcomes();
}

// Enum links of payload fields come from the header's field comments.
const enumPrefixes = [];
for (const ev of events.values()) {
  for (const pf of ev.payload) {
    if (!pf.meaning) {
      // The header comment is the fallback for a field the docs skip.
      const text = pf.comment.replace(/^\*mod:?\s*/, "");
      if (text) {
        pf.meaning = plain(text);
      } else if (pf.name !== "h") {
        warn(`${ev.name}.${pf.name} has no meaning in the docs or the header`);
      }
    }
    const en = pf.comment.match(/\b(BW_[A-Z0-9_]+_)\*/);
    if (en) {
      pf.enum = eidosName(en[1]);
      enumPrefixes.push([ev.name, pf.name, pf.enum]);
    }
    delete pf.comment;
  }
}

// ---------------------------------------------------------------- enums

// Groups of the constants, by the longest prefix. The titles are kenon's
// (curated.enumTitles), taken from the comments of eidos's mod_api.h.
const titles = curated.enumTitles;
const prefixes = Object.keys(titles).sort((a, b) => b.length - a.length);
const enums = [];
for (const c of enumsJson.constants) {
  if (/^BW_EVENT_/.test(c.name) && !/^BW_EVENT_(CONTINUE|REFUSE)$/.test(c.name)) continue;
  const prefix = prefixes.find((p) => c.name.startsWith(p));
  if (!prefix) {
    warn(`constant ${c.name} has no group in curated.enumTitles`);
    continue;
  }
  let g = enums.find((x) => x.prefix === prefix);
  if (!g) {
    g = { title: titles[prefix], prefix, members: [] };
    enums.push(g);
  }
  g.members.push({ name: eidosName(c.name), value: c.value });
}
for (const g of enums) g.prefix = eidosName(g.prefix);
for (const [ev, f, pre] of enumPrefixes) if (!enums.some((g) => g.prefix === pre)) warn(`${ev}.${f}: no enum group ${pre}`);
for (const p of Object.keys(titles)) if (!enums.some((g) => g.prefix === eidosName(p))) warn(`enum title for ${p} matches no constant`);

// ---------------------------------------------------------------- rule state

const rulesDoc = read("docs/mod-rules-language.md").split("\n");
const stateDocs = new Map();
for (const l of rulesDoc) {
  if (!isRow(l) || isRule(l)) continue;
  const c = cells(l);
  if (c.length !== 3 || c[0] === "Rule name") continue;
  for (const t of ticks(c[0])) stateDocs.set(t.replace(/\[.*$/, ""), { value: plain(c[1]), direction: c[2] });
  // A row that names a range, `move1[m]` .. `move4[m]`, covers the names between.
  const range = c[0].match(/`(\w*?)(\d+)(?:\[[^\]]*\])?` \.\. `\1(\d+)(?:\[[^\]]*\])?`/);
  if (range) {
    for (let i = Number(range[2]); i <= Number(range[3]); i++) {
      stateDocs.set(range[1] + i, { value: plain(c[1]), direction: c[2] });
    }
  }
}
// events.json says a field is writable when the port can write it; the same
// flag fills both columns.
const state = rulesJson.state_fields.map((s) => {
  const sub = s.subscript ?? "plain";
  const own = curated.state?.[s.name];
  const d = own ? { value: own } : stateDocs.get(s.name);
  if (!d) warn(`state ${s.name} has no doc row`);
  const out = {
    name: s.name,
    sub,
    writable: s.writable,
    readable: s.readable,
    writableOk: s.writable,
    subscript: { plain: null, mon: "a Pokemon, usually event.mon", int: "a number" }[sub],
    value: d?.value ?? null,
    fromC: true,
  };
  if (!s.readable) out.note = "The port cannot read this yet.";
  return out;
});

// ---------------------------------------------------------------- functions

// Declarations of eidos_wasm.h: the C signature of every import.
const decls = new Map();
for (const m of header.matchAll(/EIDOS_IMPORT\("(\w+)"\)\s+([\s\S]*?)\s+(eidos_\w+)\(([\s\S]*?)\);/g)) {
  const params = m[4].trim() === "void" ? [] : m[4].split(",").map((s) => s.replace(/\s+/g, " ").trim());
  decls.set(m[1], { ret: m[2].replace(/\s+/g, " "), cName: m[3], params });
}
const jsonCalls = new Map();
for (const [kind, j] of [["state", stateJson], ["service", servicesJson]]) {
  for (const c of j.calls) jsonCalls.set(c.name, { kind, group: "", call: c });
}
const functions = [];
const addFn = (name, kind, extra = {}) => {
  const d = decls.get(name);
  if (!d) {
    warn(`${name} is in sdk/api/v4 but not declared in eidos_wasm.h`);
    return;
  }
  const c = curated.functions[name] ?? {};
  if (!c.what) warn(`no description for ${name}`);
  functions.push({
    id: `fn:${name}`,
    kind,
    name,
    cName: d.cName,
    group: (curated.functionGroups.find((g) => g.fns.includes(name)) ?? {}).title ?? "",
    signature: `${d.ret} ${d.cName}(${d.params.join(", ") || "void"})`,
    params: extra.params ?? d.params,
    what: c.what ?? null,
    result: c.result ?? null,
    notes: c.notes ?? [],
    tiers: c.tiers ?? ["c"],
    rule: c.rule ?? null,
    events: [],
    fromC: true,
  });
};
// The core imports are only in the header, ahead of the state calls.
const hostNames = [...decls.keys()].filter((n) => !jsonCalls.has(n));
for (const n of hostNames) addFn(n, "host");
for (const [name, { kind, call }] of jsonCalls) {
  const params = call.params.map((p) => decl(cType(p.type), p.name));
  addFn(name, kind, { params });
}

// ---------------------------------------------------------------- hooks

const protos = new Map();
for (const f of ["build32/boot/gen/boot_link.c", "build32/ovl/boot/ovl_dispatch.c"]) {
  const p = join(eidos, f);
  if (!existsSync(p)) continue;
  const re = /BW_MOD_REGION_(?:ARM9|OVL\((\d+)\)),\s*0x([0-9A-Fa-f]{8})u,\s*"(\w+)",\s*(?:"([^"]*)"|NULL)/g;
  for (const m of readFileSync(p, "utf8").matchAll(re)) {
    if (m[4]) protos.set(`${m[1] ? "ov" + m[1].padStart(3, "0") : "arm9"}:${parseInt(m[2], 16)}`, m[4]);
  }
}
// v4 has no prototypes. Keep the ones the last api.json carried, so a run
// loses none.
const previous = new Map();
if (existsSync(join(outDir, "api.json"))) {
  for (const h of JSON.parse(readFileSync(join(outDir, "api.json"), "utf8")).hooks ?? []) {
    if (h.proto) previous.set(h.key, h.proto);
  }
}
const hooks = hooksJson.hooks.map((d) => {
  const n = parseInt(d.address, 16);
  const h = {
    id: `hook:${d.region}:${d.address}`,
    name: d.name,
    region: d.region,
    addr: d.address,
    key: `${d.region}:${d.address}`,
    events: d.events,
  };
  const proto = protos.get(`${d.region}:${n}`) ?? previous.get(h.key);
  if (proto) {
    h.proto = proto;
    h.protoFromC = true;
  }
  const alias = renamed.get(`${d.region}:${n}`);
  if (alias) h.corpusName = alias;
  return h;
});
for (const h of hooks) for (const e of h.events) if (!events.has(e)) warn(`hook ${h.name} names the unknown event ${e}`);

// ---------------------------------------------------------------- assemble

const need = (cond, msg) => cond || warn(msg);

// Functions worth a link from an event page, by what the event touches.
function defaultFns(e) {
  const has = (n) => e.payload.some((f) => f.name === n);
  const out = [];
  if (e.category === "Battle") out.push("battle_get", "battle_ref");
  else if (e.payload.some((f) => f.ruleKind === "mon")) out.push("party_get", "party_set");
  if (e.category === "Items and money" || e.name.startsWith("catch")) out.push("money", "money_set");
  if (e.category === "Encounters") out.push("zone", "badge_count", "species_value");
  if (e.category === "World" || e.category === "Progress") out.push("zone", "flag", "flag_set");
  if (e.category === "Session") out.push("state_load", "state_store");
  if (e.name.startsWith("box.")) out.push("box_get", "mon_to_box");
  if (has("species")) out.push("dex_caught");
  out.push("message");
  return [...new Set(out)];
}

const eventList = [...events.values()]
  .sort((a, b) => a.numId - b.numId)
  .map((e) => {
    const c = curated.events[e.name] ?? {};
    need(c.when, `no 'when' text for ${e.name}`);
    const category = c.category ?? e.category ?? "Other";
    if (!categories.includes(category)) categories.push(category);
    return {
      ...e,
      category,
      notes: [...e.notes, ...(c.facts ?? [])].map(eidosName),
      id: `event:${e.name}`,
      when: c.when ? eidosName(c.when) : null,
      functions: c.functions ?? defaultFns({ ...e, category }),
      recipes: [],
    };
  });

for (const f of functions) {
  need(curated.functions[f.name]?.what, `no description for ${f.name}`);
}
for (const name of Object.keys(curated.functions)) {
  if (!functions.some((f) => f.name === name)) warn(`curated function ${name} is not in sdk/api/v4`);
}
for (const g of curated.functionGroups) {
  for (const n of g.fns) if (!functions.some((f) => f.name === n)) warn(`function group '${g.title}' names the unknown ${n}`);
}
for (const f of functions) if (!f.group) warn(`${f.name} is in no function group`);
for (const name of Object.keys(curated.events)) if (!events.has(name)) warn(`curated event ${name} is not in sdk/api/v4`);
for (const name of Object.keys(curated.state ?? {})) if (!state.some((s) => s.name === name)) warn(`curated state ${name} is not in sdk/api/v4`);

const recipes = curated.recipes.map((r) => ({ ...r, id: `recipe:${r.slug}` }));
for (const r of recipes) {
  for (const e of r.events ?? []) {
    const ev = eventList.find((x) => x.name === e);
    if (!ev) warn(`recipe ${r.slug}: unknown event ${e}`);
    else ev.recipes.push(r.slug);
  }
  for (const f of r.functions ?? []) need(functions.some((x) => x.name === f), `recipe ${r.slug}: unknown function ${f}`);
}
for (const e of eventList) {
  for (const f of e.functions) {
    const fn = functions.find((x) => x.name === f);
    if (!fn) warn(`${e.name}: unknown function ${f}`);
    else fn.events.push(e.name);
  }
}

let rev = null;
try {
  rev = execFileSync("git", ["-C", eidos, "rev-parse", "--short", "HEAD"], { encoding: "utf8" }).trim();
} catch {
  // not a git checkout
}

const out = {
  meta: {
    generatedBy: "tools/mod-docs.mjs",
    eidosRevision: rev,
    apiVersion,
    sources: [
      "sdk/api/v4/events.json",
      "sdk/api/v4/rules.json",
      "sdk/api/v4/state.json",
      "sdk/api/v4/services.json",
      "sdk/api/v4/enums.json",
      "sdk/api/v4/hooks.json",
      "sdk/api/v4/eidos_wasm.h",
      "docs/mod-events.md",
      "docs/mod-payloads.md",
      "docs/mod-rules-language.md",
      "ui/src/lib/moddocs/curated.mjs",
    ],
    note: "The structure is eidos's sdk/api/v4. The words are eidos's docs and kenon's curated text. Hook prototypes are kept from earlier builds.",
  },
  categories,
  events: eventList,
  enums,
  state,
  actions: curated.actions,
  functions,
  functionGroups: curated.functionGroups,
  hooks,
  hookPage: { ...curated.hookPage, id: "page:hooks" },
  hookFamilies: curated.hookFamilies,
  pages: curated.pages.map((x) => ({ section: "rules", ...x, id: `page:${x.slug}` })),
  recipes,
};
writeFileSync(join(outDir, "api.json"), JSON.stringify(out, null, 1) + "\n");
console.log(
  `events ${eventList.length}, functions ${functions.length}, hooks ${hooks.length} (${hooks.filter((h) => h.proto).length} with a prototype), state ${state.length}, recipes ${recipes.length}`,
);
for (const w of warnings) console.warn("warning: " + w);
