// The mod docs: the generated data (api.json, from tools/mod-docs.mjs), the
// entries the modal lists and searches, and the hover text for the editors.

export interface PayloadField {
  name: string;
  type: string;
  meaning: string;
  writable: boolean;
  cName?: string;
  cType?: string;
  ruleKind?: string;
  enum?: string;
}
export interface ApiEvent {
  id: string;
  name: string;
  numId: number;
  category: string;
  /** The API version that added the event. */
  since: number;
  /** The game functions the event fires from. */
  where: string;
  refuse: string | null;
  refusable?: boolean;
  modifyText: string | null;
  when: string | null;
  notes: string[];
  payload: PayloadField[];
  functions: string[];
  recipes: string[];
  ruleEvent?: boolean;
  cStruct?: string;
}
export interface EnumGroup {
  title: string;
  prefix: string;
  members: { name: string; value: number }[];
}
export interface StateField {
  name: string;
  sub: string;
  subscript: string | null;
  writable: boolean;
  readable: boolean;
  writableOk: boolean;
  value: string | null;
  note?: string;
}
export interface RuleAction {
  name: string;
  syntax: string;
  what: string;
  example: string;
}
export interface ApiFunction {
  id: string;
  kind: "host" | "state" | "service";
  name: string;
  cName?: string;
  group: string;
  signature: string;
  params: string[];
  what: string | null;
  result: string | null;
  notes: string[];
  tiers: string[];
  rule: string | null;
  events: string[];
}
export interface Hook {
  id: string;
  name: string;
  region: string;
  addr: string;
  key: string;
  events: string[];
  proto?: string;
  /** Other names the function has had. */
  aliases: string[];
}
export type Block =
  | { t: "p"; text: string }
  | { t: "code"; lang: string; text: string }
  | { t: "list"; items: string[] }
  | { t: "table"; head: string[]; rows: string[][] };
export interface Page {
  id: string;
  section?: string;
  slug?: string;
  title: string;
  blocks: Block[];
}
export interface Recipe {
  id: string;
  slug: string;
  title: string;
  goal: string;
  tier: string[];
  events: string[];
  functions: string[];
  steps: string[];
  rule?: string;
  rule2?: string;
  c?: string;
  settings?: string;
  /** Table files the recipe reads, with their text. */
  tables?: { file: string; text: string }[];
  notes?: string[];
}
export interface Api {
  meta: { eidosRevision: string | null; apiVersion: number };
  categories: string[];
  events: ApiEvent[];
  enums: EnumGroup[];
  state: StateField[];
  actions: RuleAction[];
  functions: ApiFunction[];
  functionGroups: { title: string; fns: string[] }[];
  hooks: Hook[];
  hookPage: Page;
  hookFamilies: [string, string][];
  pages: Page[];
  recipes: Recipe[];
}

let cached: Promise<Api> | null = null;
export function loadApi(): Promise<Api> {
  cached ??= import("./api.json").then((m) => (m.default ?? m) as unknown as Api);
  return cached;
}

export type Section = "events" | "functions" | "hooks" | "rules" | "recipes";
export const SECTIONS: { id: Section; title: string }[] = [
  { id: "events", title: "Events" },
  { id: "functions", title: "Functions" },
  { id: "hooks", title: "Hookable game functions" },
  { id: "rules", title: "Rules language" },
  { id: "recipes", title: "Recipes" },
];

export interface Entry {
  id: string;
  section: Section;
  group: string;
  title: string;
  /** Added in API 4. */
  isNew?: boolean;
}

export function hookFamily(api: Api, name: string): string {
  const hit = api.hookFamilies.find(([prefix]) => name.startsWith(prefix));
  return hit ? hit[1] : "Other";
}

export function buildEntries(api: Api): Entry[] {
  const out: Entry[] = [];
  for (const e of api.events) {
    out.push({
      id: e.id,
      section: "events",
      group: e.category,
      title: e.name,
      isNew: e.since >= 4,
    });
  }
  for (const p of api.pages.filter((x) => x.section === "functions")) {
    out.push({ id: p.id, section: "functions", group: "Code mods", title: p.title });
  }
  for (const f of api.functions) {
    out.push({ id: f.id, section: "functions", group: f.group, title: f.name });
  }
  out.push({ id: api.hookPage.id, section: "hooks", group: "Raw hooks", title: api.hookPage.title });
  for (const h of api.hooks) {
    out.push({ id: h.id, section: "hooks", group: hookFamily(api, h.name), title: h.name });
  }
  for (const p of api.pages.filter((x) => x.section === "rules")) {
    out.push({ id: p.id, section: "rules", group: "Language", title: p.title });
  }
  out.push({ id: "page:actions", section: "rules", group: "Language", title: "Actions" });
  for (const s of api.state) {
    out.push({ id: `state:${s.name}`, section: "rules", group: "State fields", title: s.name });
  }
  for (const r of api.recipes) {
    out.push({ id: r.id, section: "recipes", group: r.tier.includes("rules") ? "Rules" : "Code", title: r.title });
  }
  return out;
}

/** The id of the entry a context names: an event, a function or a hook. */
export function resolve(api: Api, name: string): string | null {
  const ev = api.events.find((e) => e.name === name);
  if (ev) return ev.id;
  const fn = api.functions.find((f) => f.name === name);
  if (fn) return fn.id;
  return null;
}

// ---------------------------------------------------------------- snippets

const slug = (name: string) => name.replace(/[^a-z0-9]+/gi, "_").toLowerCase();

/** A rule that binds the event, with a guard on its first number field. */
export function ruleSkeleton(e: ApiEvent): string {
  const num = e.payload.find((f) => f.ruleKind === "int" && !f.writable) ?? e.payload.find((f) => f.ruleKind === "int");
  const writable = e.payload.find((f) => f.writable && f.ruleKind === "int");
  const lines = ["language 1", `rule my_${slug(e.name)} on ${e.name}`];
  if (num) lines.push(`    when event.${num.name} == 0`);
  if (writable) lines.push(`    do set event.${writable.name} event.${writable.name}`);
  else if (e.refusable) lines.push("    do refuse");
  else lines.push('    do message "Hello"');
  return lines.join("\n") + "\n";
}

/** A C mod (eidos_wasm.h) with one handler for the event. */
export function cSkeleton(e: ApiEvent): string {
  const id = `EIDOS_EVENT_${e.name.toUpperCase().replace(/\./g, "_")}`;
  const fn = `on_${slug(e.name)}`;
  const lines = ['#include "eidos_wasm.h"', "", `static int32_t ${fn}(EidosEvent *event)`, "{"];
  if (e.cStruct) {
    lines.push(`    ${e.cStruct} *e = (${e.cStruct} *)event;`, "    (void)e;");
  } else {
    lines.push("    (void)event;");
  }
  lines.push("    return EIDOS_EVENT_CONTINUE;", "}", "");
  lines.push(
    'EIDOS_EXPORT("eidos_mod_start")',
    "int32_t eidos_mod_start(void)",
    "{",
    `    return eidos_on(${id}, (uint32_t)(uintptr_t)${fn});`,
    "}",
    "",
    'EIDOS_EXPORT("eidos_mod_stop")',
    "void eidos_mod_stop(void)",
    "{",
    "}",
  );
  return lines.join("\n") + "\n";
}

/** The override line for a hook. */
export function hookLine(h: Hook): string {
  const sig = h.proto ? ` sig="${h.proto}"` : "";
  return `${h.key} wrap my_${slug(h.name)}${sig}\n`;
}

// ---------------------------------------------------------------- hover

/** Markdown for a word in an editor, or null. Names an event, a payload
 * struct, a service or state call, or an enum constant. */
export function hoverFor(api: Api, word: string): string | null {
  const evName = (w: string) => {
    const m = w.match(/^(?:BW|EIDOS)_EVENT_([A-Z_]+)$/);
    if (!m) return null;
    return api.events.find((e) => e.name.toUpperCase().replace(/\./g, "_") === m[1]) ?? null;
  };
  const ev = evName(word) ?? api.events.find((e) => e.cStruct === word) ?? null;
  if (ev) {
    const fields = ev.payload.map((f) => `- \`${f.cName ?? f.name}\` ${f.type}${f.writable ? " (writable)" : ""}: ${f.meaning}`).join("\n");
    return `**${ev.name}** (${ev.category}${ev.refusable ? ", can be refused" : ""})\n\n${ev.when ?? ""}\n\n${fields}`;
  }
  const fn = api.functions.find((f) => f.name === word || f.cName === word);
  if (fn) return `**${fn.name}**\n\n\`${fn.signature}\`\n\n${fn.what ?? ""}${fn.result ? `\n\nReturns: ${fn.result}` : ""}`;
  for (const g of api.enums) {
    const m = g.members.find((x) => x.name === word);
    if (m) return `\`${m.name}\` = ${m.value}\n\n${g.title}`;
  }
  return null;
}

/** Completion items for the editors: the services and the state calls. */
export function completionDocs(api: Api): { name: string; detail: string; doc: string }[] {
  return api.functions.map((f) => ({ name: f.name, detail: f.signature, doc: f.what ?? "" }));
}
