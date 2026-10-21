// Builds ui/src/lib/moddocs/api.json from eidos's published API description
// (sdk/api/v4) plus kenon's own text (ui/src/lib/moddocs/curated.mjs).
//
//   node tools/mod-docs.mjs [--eidos <folder>]
//
// The eidos folder is --eidos, then $EIDOS, then ../eidos. Point it at a
// checkout of the commit you want (git worktree add <dir> <commit>).
//
// Everything comes from sdk/api/v4: events with their game function, refusal
// outcome, writable fields and field meanings; rule names and state fields;
// the core imports; services and state calls; constants; hooks with their
// prototypes. eidos_wasm.h supplies the C signature of each call. curated.mjs
// holds only what v4 does not carry: extra facts and linked calls of an event,
// what each call does, rules-language pages, recipes and group names.

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

// ---------------------------------------------------------------- v4 files

const coreJson = v4("core.json");
const eventsJson = v4("events.json");
const rulesJson = v4("rules.json");
const stateJson = v4("state.json");
const servicesJson = v4("services.json");
const enumsJson = v4("enums.json");
const hooksJson = v4("hooks.json");
const header = read("sdk/api/v4/eidos_wasm.h");
const apiVersion = eventsJson.api;
for (const [n, j] of [["core", coreJson], ["rules", rulesJson], ["state", stateJson], ["services", servicesJson], ["enums", enumsJson], ["hooks", hooksJson]]) {
  if (j.api !== apiVersion) warn(`${n}.json is api ${j.api}, events.json is api ${apiVersion}`);
}

const curated = (await import(pathToFileURL(join(outDir, "curated.mjs")).href)).default;

// ---------------------------------------------------------------- events

// The text is shown without code ticks.
const prose = (t) => t.replace(/`/g, "").trim();
// A sentence: first letter up, a full stop at the end.
const sentenceCase = (t) => (t ? t.charAt(0).toUpperCase() + t.slice(1) + (/[.!?]$/.test(t) ? "" : ".") : t);

const ruleEventNames = new Set(rulesJson.events);
// The groups, v4's own, in the order the events list them.
const titleCase = (c) => c.charAt(0).toUpperCase() + c.slice(1);
const categories = [];
for (const e of eventsJson.events) if (!categories.includes(titleCase(e.category))) categories.push(titleCase(e.category));

const events = new Map();
for (const e of eventsJson.events) {
  const cStruct = e.struct;
  const payload = e.fields.map((f) => {
    const field = {
      name: f.rule_name,
      type: f.type + (f.count !== null ? `[${f.count}]` : ""),
      meaning: prose(f.doc ?? ""),
      writable: f.writable,
      cName: f.name,
      cType: f.c_type + (f.count !== null ? `[${f.count}]` : ""),
      ruleKind: f.count !== null ? "list" : f.type === "EidosMonRef" ? "mon" : "int",
    };
    return field;
  });
  events.set(e.name, {
    name: e.name,
    category: titleCase(e.category),
    since: e.since,
    where: prose(e.where ?? ""),
    refuse: e.refuse ? prose(e.refuse) : null,
    modifyText: e.modify ? prose(e.modify) : null,
    doc: prose(e.doc ?? ""),
    details: prose(e.details ?? ""),
    notes: [],
    payload,
    cStruct,
    numId: e.id,
    refusable: e.refusable,
    ruleEvent: ruleEventNames.has(e.name),
  });
}
for (const n of ruleEventNames) if (!events.has(n)) warn(`rules.json names ${n}, events.json has no such event`);
for (const ev of events.values()) {
  if ((ev.refuse !== null) !== ev.refusable) warn(`${ev.name}: events.json says refusable ${ev.refusable}, refuse is ${ev.refuse}`);
}

// ---------------------------------------------------------------- enums

// Groups of the constants, by the longest prefix. The titles are kenon's
// (curated.enumTitles), taken from the comments of eidos's mod_api.h.
const titles = curated.enumTitles;
const prefixes = Object.keys(titles).sort((a, b) => b.length - a.length);
const enums = [];
for (const c of enumsJson.constants) {
  if (/^EIDOS_EVENT_/.test(c.name) && !/^EIDOS_EVENT_(CONTINUE|REFUSE)$/.test(c.name)) continue;
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
  g.members.push({ name: c.name, value: c.value });
}
// A field whose doc starts with a constant (EIDOS_FORM_FIELD) links to the group the
// constant belongs to, found by the longest group prefix.
for (const ev of events.values()) {
  for (const f of ev.payload) {
    const named = f.meaning.match(/^EIDOS_[A-Z0-9_]+/g) ?? [];
    for (const n of named) {
      const g = enums.filter((x) => n.startsWith(x.prefix)).sort((x, y) => y.prefix.length - x.prefix.length)[0];
      if (g) {
        f.enum = g.prefix;
        break;
      }
    }
  }
}
for (const p of Object.keys(titles)) if (!enums.some((g) => g.prefix === p)) warn(`enum title for ${p} matches no constant`);

// ---------------------------------------------------------------- rule state

// writable_declared is what the rule language declares; writable is what the
// port really writes.
const state = rulesJson.state_fields.map((s) => {
  const sub = s.subscript ?? "plain";
  const value = s.doc ? sentenceCase(prose(s.doc)) : null;
  if (!value) warn(`state ${s.name} has no text`);
  const out = {
    name: s.name,
    sub,
    writable: s.writable_declared,
    readable: s.readable,
    writableOk: s.writable,
    subscript: { plain: null, mon: "a Pokemon, usually event.mon", int: "a number" }[sub],
    value: value ?? null,
    fromC: true,
  };
  const notes = [];
  if (!s.readable) notes.push("The port cannot read this yet.");
  if (s.writable_declared && !s.writable) notes.push("It cannot be changed yet.");
  if (notes.length) out.note = notes.join(" ");
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
// A core.json note is a lower-case phrase; the docs show a sentence.
const sentence = (t) => t.charAt(0).toUpperCase() + t.slice(1) + (t.endsWith(".") ? "" : ".");
const addFn = (name, kind) => {
  const d = decls.get(name);
  if (!d) {
    warn(`${name} is in sdk/api/v4 but not declared in eidos_wasm.h`);
    return;
  }
  const c = curated.functions[name] ?? {};
  functions.push({
    id: `fn:${name}`,
    kind,
    name,
    cName: d.cName,
    group: (curated.functionGroups.find((g) => g.fns.includes(name)) ?? {}).title ?? "",
    signature: `${d.ret} ${d.cName}(${d.params.join(", ") || "void"})`,
    params: d.params,
    what: c.what ?? (coreDoc.get(name) ? sentence(coreDoc.get(name)) : null),
    result: c.result ?? null,
    notes: c.notes ?? [],
    tiers: c.tiers ?? ["c"],
    rule: c.rule ?? null,
    events: [],
    fromC: true,
  });
};
// The core imports come from core.json, ahead of the state calls.
const coreDoc = new Map(coreJson.imports.map((i) => [i.name, i.doc]));
for (const i of coreJson.imports) addFn(i.name, "host");
for (const [name, { kind }] of jsonCalls) addFn(name, kind);

// ---------------------------------------------------------------- hooks

// v4's signature is the type-only form a hook line states, "int (void *)".
const hooks = hooksJson.hooks.map((d) => ({
  id: `hook:${d.region}:${d.address}`,
  name: d.name,
  region: d.region,
  addr: d.address,
  key: `${d.region}:${d.address}`,
  events: d.events,
  proto: d.signature,
  aliases: d.aliases ?? [],
  protoFromC: true,
}));
for (const h of hooks) for (const e of h.events) if (!events.has(e)) warn(`hook ${h.name} names the unknown event ${e}`);

// ---------------------------------------------------------------- assemble

const need = (cond, msg) => cond || warn(msg);

// Functions worth a link from an event page, by what the event touches.
function defaultFns(e) {
  const has = (n) => e.payload.some((f) => f.name === n);
  const out = [];
  if (e.category === "Battle") out.push("battle_get", "battle_ref");
  else if (e.payload.some((f) => f.ruleKind === "mon")) out.push("party_get", "party_set");
  if (e.category === "Items, money and healing" || e.name.startsWith("catch")) out.push("money", "money_set");
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
    // v4's sentence says when the event fires; its details follow it.
    const when = e.doc;
    need(when, `no 'when' text for ${e.name}`);
    return {
      ...e,
      doc: undefined,
      notes: c.facts ?? [],
      id: `event:${e.name}`,
      when: when || null,
      details: e.details || null,
      functions: c.functions ?? defaultFns(e),
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

// The "Code mods" page lists the core imports and exports of core.json.
const pages = curated.pages.map((x) => ({ section: "rules", ...x, id: `page:${x.slug}` }));
const codePage = pages.find((p) => p.slug === "code-mods");
if (codePage) {
  const core = coreJson.imports.map((i) => functions.find((f) => f.name === i.name)).filter(Boolean);
  codePage.blocks.push(
    { t: "p", text: `The core imports come from the module \`${coreJson.import_module}\`.` },
    { t: "table", head: ["Call", "What it does"], rows: core.map((f) => [`\`${f.signature}\``, f.what ?? ""]) },
    { t: "p", text: "The module exports:" },
    { t: "list", items: coreJson.exports.map((x) => `\`${x.name}\`${x.doc ? ": " + sentence(x.doc) : ""}`) },
  );
}

const out = {
  meta: {
    generatedBy: "tools/mod-docs.mjs",
    eidosRevision: rev,
    apiVersion,
    sources: [
      "sdk/api/v4/core.json",
      "sdk/api/v4/events.json",
      "sdk/api/v4/rules.json",
      "sdk/api/v4/state.json",
      "sdk/api/v4/services.json",
      "sdk/api/v4/enums.json",
      "sdk/api/v4/hooks.json",
      "sdk/api/v4/eidos_wasm.h",
      "ui/src/lib/moddocs/curated.mjs",
    ],
    note: "The structure and the words are eidos's sdk/api/v4. kenon's curated text fills what v4 does not carry.",
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
  pages,
  recipes,
};
writeFileSync(join(outDir, "api.json"), JSON.stringify(out, null, 1) + "\n");
console.log(
  `events ${eventList.length}, functions ${functions.length}, hooks ${hooks.length} (${hooks.filter((h) => h.proto).length} with a prototype), state ${state.length}, recipes ${recipes.length}`,
);
for (const w of warnings) console.warn("warning: " + w);
