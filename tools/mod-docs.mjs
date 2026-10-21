// Builds ui/src/lib/moddocs/api.json from eidos's docs and headers plus
// kenon's own text (ui/src/lib/moddocs/curated.mjs).
//
//   node tools/mod-docs.mjs [--eidos <folder>]
//
// The eidos folder is --eidos, then $EIDOS, then ../eidos. When eidos
// publishes sdk/api/v4 (events.json, services.json, hooks.json), replace the
// readers below with readers of those files and keep the output shape.
// Anything read from C or build output instead of the docs carries
// `fromC: true`.

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
  s
    .replace(/`/g, "")
    .replace(/\*\*/g, "")
    .replace(/\*([^*]+)\*/g, "$1")
    .trim();

// Bullets of a block: "- text" with indented continuation lines.
function bullets(lines) {
  const out = [];
  for (const l of lines) {
    if (l.startsWith("- ")) out.push(l.slice(2).trim());
    else if (out.length && /^\s{2,}\S/.test(l)) out[out.length - 1] += " " + l.trim();
  }
  return out;
}

// ---------------------------------------------------------------- events

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
const events = new Map();
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
      events.set(names[0], {
        name: names[0],
        category: cat,
        binding: plain(c[1]),
        region: plain(c[2]),
        refuse: c[3] === "-" ? null : plain(c[3]),
        modifyText: c[4] === "-" ? null : plain(c[4]),
        notes: [],
        caveats: [],
        payload: [],
      });
    }
  }
  flush();
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

// ---------------------------------------------------------------- payloads

{
  const lines = read("docs/mod-payloads.md").split("\n");
  const start = lines.findIndex((l) => l.startsWith("## 2."));
  let names = null;
  for (let i = start + 1; i < lines.length; i++) {
    const l = lines[i];
    if (l.startsWith("## ")) break;
    if (l.startsWith("**`")) {
      names = ticks(l.split(":")[0]).filter((n) => events.has(n));
      if (!names.length) names = null;
      continue;
    }
    if (names && isRow(l) && !isRule(l)) {
      const c = cells(l);
      if (c[0] === "Field") continue;
      const fieldNames = ticks(c[0]);
      const writable = /\*mod\*/.test(c[1]);
      const type = c[1].replace(/,?\s*\*mod\*/, "").replace(/`/g, "").trim();
      const only = c[2].match(/^`?([a-z]+.[a-z]+)`? only/);
      for (const n of names) {
        if (only && only[1] !== n) continue;
        for (const f of fieldNames) {
          events.get(n).payload.push({ name: f, type, meaning: plain(c[2]), writable });
        }
      }
    } else if (names && l.trim() && !isRow(l)) {
      names = null;
    }
  }
}

// ---------------------------------------------------------------- mod_api.h

const apiH = read("sdk/mod_api.h");
const stateH = read("sdk/mod_state.h");
const eventIds = new Map();
for (const m of apiH.matchAll(/BW_EVENT_([A-Z_]+)\s*=\s*(\d+)/g)) eventIds.set(m[1], Number(m[2]));

const enums = [];
for (const m of apiH.matchAll(/\/\*\s*([^*]*?)\s*\*\/\s*enum\s*\{([\s\S]*?)\};/g)) {
  const members = [...m[2].matchAll(/(BW_[A-Z0-9_]+)\s*=\s*(0x[0-9A-Fa-f]+|\d+)/g)].map((x) => ({
    name: x[1],
    value: Number(x[2]),
  }));
  if (!members.length || members[0].name.startsWith("BW_EVENT_")) continue;
  const prefix = members[0].name.replace(/_[A-Z0-9]+$/, "_");
  enums.push({ title: m[1].replace(/\s+/g, " "), prefix, members, fromC: true });
}
for (const m of stateH.matchAll(/enum\s*\{([\s\S]*?)\};/g)) {
  const members = [...m[1].matchAll(/(BW_[A-Z0-9_]+)\s*=\s*(0x[0-9A-Fa-f]+|\d+)u?/g)].map((x) => ({
    name: x[1],
    value: Number(x[2]),
  }));
  if (!members.length) continue;
  const first = members[0].name;
  const prefix = first.replace(/_[A-Z0-9]+$/, "_");
  const title = first.startsWith("BW_MON_FIELD")
    ? "Pokemon field ids (party_get, party_set, box_get, box_set, battle_get, battle_set)"
    : first.startsWith("BW_SESSION")
      ? "What a session handler is told about"
      : "The flags of a run restart";
  enums.push({ title, prefix, members, fromC: true });
}

// Struct fields, for the C names and the enum links.
const structs = new Map();
for (const m of apiH.matchAll(/typedef struct (Bw\w+) \{([\s\S]*?)\} \1;/g)) {
  const fields = [];
  for (const line of m[2].split("\n")) {
    const f = line.match(/^\s*((?:const )?\w+)\s+([^;/]+);\s*(?:\/\*\s*(.*?)\s*\*\/)?/);
    if (!f || f[1] === "BwEvent") continue;
    for (const part of f[2].split(",")) {
      const nm = part.trim().match(/^(\w+)(\[\d+\])?/);
      if (nm) fields.push({ cName: nm[1], cType: f[1] + (nm[2] ?? ""), comment: f[3] ?? "" });
    }
  }
  structs.set(m[1].toLowerCase().replace(/^bw/, ""), { cStruct: m[1], fields });
}

for (const ev of events.values()) {
  const key = ev.name.replace(/[._]/g, "");
  const st = structs.get(key);
  const id = eventIds.get(ev.name.toUpperCase().replace(/\./g, "_"));
  if (id === undefined) warn(`no id for ${ev.name}`);
  ev.id = id;
  if (!st) {
    warn(`no struct for ${ev.name}`);
    continue;
  }
  ev.cStruct = st.cStruct;
  for (const pf of ev.payload) {
    const cf = st.fields.find((f) => f.cName === pf.name || f.cName === pf.name + "_");
    if (!cf) {
      warn(`${ev.name}.${pf.name} is not in ${st.cStruct}`);
      continue;
    }
    pf.cName = cf.cName;
    pf.cType = cf.cType;
    if (/\*mod/.test(cf.comment) !== pf.writable) warn(`${ev.name}.${pf.name}: docs and header disagree on *mod*`);
    const en = cf.comment.match(/(BW_[A-Z0-9]+_)\*/);
    if (en && enums.some((e) => e.prefix === en[1])) pf.enum = en[1];
  }
  for (const cf of st.fields) {
    if (!ev.payload.some((p) => p.cName === cf.cName)) warn(`${ev.name}: ${cf.cName} is in the header, not in the docs`);
  }
}

// ---------------------------------------------------------------- rules.txt

const rulesTxt = join(root, "core", "src", "modnames", "rules.txt");
const ruleEvents = new Map();
const stateFields = [];
let lastEvent = null;
for (const raw of readFileSync(rulesTxt, "utf8").split(/\r?\n/)) {
  const l = raw.trim().split(/\s+/);
  if (l[0] === "event") {
    lastEvent = { refusable: l[2] === "1", fields: [] };
    ruleEvents.set(l[1], lastEvent);
  } else if (l[0] === "field") lastEvent.fields.push({ name: l[1], kind: l[2], writable: (l[3] ?? "").includes("m") });
  else if (l[0] === "state")
    stateFields.push({ name: l[1], sub: l[2], writable: l[3] === "1", readable: l[4] === "1", writableOk: l[5] === "1" });
}
for (const ev of events.values()) {
  const re = ruleEvents.get(ev.name);
  if (!re) {
    warn(`${ev.name} is not a rule event`);
    ev.ruleEvent = false;
    continue;
  }
  ev.ruleEvent = true;
  ev.refusable = re.refusable;
  if (re.refusable !== (ev.refuse !== null)) warn(`${ev.name}: docs and the rule table disagree on refusing`);
  for (const pf of ev.payload) {
    const rf = re.fields.find((f) => f.name === pf.name);
    if (rf) pf.ruleKind = rf.kind;
    else warn(`${ev.name}.${pf.name} is not a rule field`);
  }
}

// ---------------------------------------------------------------- rule state

const rulesDoc = read("docs/mod-rules-language.md").split("\n");
const stateDocs = new Map();
for (const l of rulesDoc) {
  if (!isRow(l) || isRule(l)) continue;
  const c = cells(l);
  if (c.length !== 3 || c[0] === "Rule name") continue;
  for (const t of ticks(c[0])) stateDocs.set(t.replace(/\[.*$/, ""), { value: plain(c[1]), direction: c[2] });
}
const state = stateFields.map((s) => {
  const d = stateDocs.get(s.name);
  if (!d) warn(`state ${s.name} has no doc row`);
  const out = {
    ...s,
    subscript: { plain: null, mon: "a Pokemon, usually event.mon", int: "a number" }[s.sub],
    value: d?.value ?? null,
    fromC: true,
  };
  if (!s.readable) out.note = "The port cannot read this yet.";
  else if (s.writable && !s.writableOk) out.note = "The port cannot write this yet.";
  return out;
});

// ---------------------------------------------------------------- services

function parseTable(src, struct) {
  const body = src.match(new RegExp(`typedef struct ${struct} \\{([\\s\\S]*?)\\} ${struct};`))[1];
  const out = [];
  let group = "";
  for (const line of body.split("\n")) {
    const g = line.match(/^\s*\/\*\s*(.*?)\s*\*\/\s*$/);
    if (g && !line.includes("(*")) {
      group = g[1].replace(/\.$/, "");
      continue;
    }
    const f = line.match(/^\s*(\w[\w\s*]*?)\s*\(\*(\w+)\)\s*\((.*)\)\s*;/);
    if (f) {
      out.push({
        group,
        name: f[2],
        returns: f[1].trim(),
        params: f[3].trim() === "void" ? [] : f[3].split(",").map((s) => s.trim()),
      });
    }
  }
  return out;
}
const stateCalls = parseTable(stateH, "BwModState");
const serviceCalls = parseTable(stateH, "BwModServices");
const hostCalls = parseTable(apiH, "BwModHost");

// ---------------------------------------------------------------- hooks

const declared = [];
for (const raw of read("mod/declared_set.txt").split("\n")) {
  if (!raw.trim() || raw.startsWith("#")) continue;
  const c = raw.split("\t");
  if (c.length < 3) continue;
  declared.push({
    addr: c[0].trim(),
    name: c[1].trim(),
    region: c[2].trim(),
    events: (c[3] ?? "")
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean),
  });
}
const protos = new Map();
for (const f of ["build32/boot/gen/boot_link.c", "build32/ovl/boot/ovl_dispatch.c"]) {
  const p = join(eidos, f);
  if (!existsSync(p)) continue;
  const re = /BW_MOD_REGION_(?:ARM9|OVL\((\d+)\)),\s*0x([0-9A-Fa-f]{8})u,\s*"(\w+)",\s*(?:"([^"]*)"|NULL)/g;
  for (const m of readFileSync(p, "utf8").matchAll(re)) {
    if (m[4]) protos.set(`${m[1] ? "ov" + m[1].padStart(3, "0") : "arm9"}:${parseInt(m[2], 16)}`, m[4]);
  }
}
const hooks = declared.map((d) => {
  const n = parseInt(d.addr, 16);
  const h = { id: `hook:${d.region}:${d.addr}`, name: d.name, region: d.region, addr: d.addr, key: `${d.region}:${d.addr}`, events: d.events };
  const proto = protos.get(`${d.region}:${n}`);
  if (proto) {
    h.proto = proto;
    h.protoFromC = true;
  }
  const alias = renamed.get(`${d.region}:${n}`);
  if (alias) h.corpusName = alias;
  return h;
});

// ---------------------------------------------------------------- assemble

const curated = (await import(pathToFileURL(join(outDir, "curated.mjs")).href)).default;
const need = (cond, msg) => cond || warn(msg);

// Functions worth a link from an event page, by what the event touches.
function defaultFns(e) {
  const has = (n) => e.payload.some((f) => f.name === n);
  const out = [];
  if (e.category === "Battle") out.push("battle_get", "battle_ref");
  else if (e.payload.some((f) => f.type === "BwMonRef")) out.push("party_get", "party_set");
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
  .sort((a, b) => (a.id ?? 0) - (b.id ?? 0))
  .map((e) => {
    const c = curated.events[e.name] ?? {};
    need(c.when, `no 'when' text for ${e.name}`);
    return { ...e, notes: [...e.notes, ...(c.facts ?? [])], id: `event:${e.name}`, name: e.name, numId: e.id, when: c.when ?? null, functions: c.functions ?? defaultFns(e), recipes: [] };
  });

const functions = [];
const addFns = (list, kind) => {
  for (const f of list) {
    const c = curated.functions[f.name] ?? {};
    need(c.what, `no description for ${f.name}`);
    functions.push({
      id: `fn:${f.name}`,
      kind,
      name: f.name,
      group: (curated.functionGroups.find((g) => g.fns.includes(f.name)) ?? {}).title ?? f.group,
      signature: `${f.returns} (*${f.name})(${f.params.join(", ")})`,
      params: f.params,
      what: c.what ?? null,
      result: c.result ?? null,
      notes: c.notes ?? [],
      tiers: c.tiers ?? ["c"],
      rule: c.rule ?? null,
      events: [],
      fromC: true,
    });
  }
};
addFns(hostCalls, "host");
addFns(stateCalls, "state");
addFns(serviceCalls, "service");

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
    apiVersion: Number(apiH.match(/#define BW_MOD_API_VERSION (\d+)/)[1]),
    sources: [
      "docs/mod-events.md",
      "docs/mod-payloads.md",
      "docs/mod-rules-language.md",
      "sdk/mod_api.h",
      "sdk/mod_state.h",
      "mod/declared_set.txt",
      "core/src/modnames/rules.txt",
      "ui/src/lib/moddocs/curated.mjs",
    ],
    note: "Items marked fromC were read from C headers or build output, not from the docs.",
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
