// Ranked search over the mod docs. The index is built once when the docs load.
// Words are split (battle.faint -> battle, faint; BwMonRef -> bw, mon, ref),
// lower-cased and lightly stemmed. Scoring is BM25 over weighted fields, with
// prefix matching on the last word, typo tolerance, a synonym table and a
// bonus for exact phrases. Every word of the query must match somewhere.

import type { Api, Entry } from "./index";

// ------------------------------------------------------------------ synonyms

// How people describe things on the left, the API words on the right.
// A key can be one word or a phrase of up to three. Add rows freely.
const SYNONYMS: [string, string][] = [
  ["die dies died dead death dying knock out ko", "faint"],
  ["kill remove delete discard", "release"],
  ["party member", "party"],
  ["money cash prize gold coin coins funds payout reward pay", "money"],
  ["heal healing hp potion restore recover health", "heal"],
  ["catch caught capture catching pokeball", "catch"],
  ["level cap", "level exp cap"],
  ["max level", "level exp"],
  ["level limit", "level exp"],
  ["experience xp", "exp"],
  ["route map place location town city", "area zone route"],
  ["gym", "badge"],
  ["shininess", "shiny"],
  ["opponent", "trainer"],
  ["bag inventory", "item"],
  ["give gift grant award", "add receive"],
  ["pc storage", "box pc"],
  ["rename", "nickname name"],
  ["lookup", "table"],
  ["evolution evolving", "evolve"],
  ["teach learn tm hm", "learn move"],
  ["wild grass spawn", "wild encounter"],
  ["buy purchase mart", "shop buy"],
  ["flee run away", "escape"],
  ["pokemon monster creature", "mon pokemon"],
  ["dialog text say", "message"],
  ["fishing", "fish"],
];

const STOP = new Set(
  "the a an of to in on at for with and or is are be when i my me how do does can it that this what all by from into".split(" "),
);

// -------------------------------------------------------------------- tokens

const WORD = /[A-Za-z0-9]+/g;

/** The pieces of one word: camel case is split, words with digits are not. */
function splitWord(w: string): string[] {
  if (/\d/.test(w)) return [w.toLowerCase()];
  return w
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .toLowerCase()
    .split(" ");
}

/** Light stemming: plural, -ing, -ed and a trailing e go. */
export function stem(t: string): string {
  if (t.length < 4 || /\d/.test(t)) return t;
  if (t.endsWith("ies") && t.length > 4) t = t.slice(0, -3) + "y";
  else if (/(s|x|z|ch|sh)es$/.test(t) && t.length > 4) t = t.slice(0, -2);
  else if (t.endsWith("s") && !t.endsWith("ss") && !t.endsWith("us")) t = t.slice(0, -1);
  if (t.endsWith("ing") && t.length > 5) t = t.slice(0, -3);
  else if (t.endsWith("ed") && t.length > 4) t = t.slice(0, -2);
  if (t.endsWith("e") && t.length > 3) t = t.slice(0, -1);
  return t;
}

function stems(text: string): string[] {
  const out: string[] = [];
  for (const m of text.matchAll(WORD)) for (const p of splitWord(m[0])) out.push(stem(p));
  return out;
}

// Synonym table keyed by the stemmed phrase.
const SYN = new Map<string, string[]>();
let longest = 1;
for (const [keys, vals] of SYNONYMS) {
  const words = vals.split(" ").map(stem);
  // Keys are space separated; a phrase is written as consecutive words, so
  // the lists above give each single-word key and one phrase per row when
  // the row has no single-word meaning (marked by the phrase list below).
  for (const k of phraseKeys(keys)) {
    SYN.set(k.map(stem).join(" "), words);
    longest = Math.max(longest, k.length);
  }
}

/** A key list is single words, except for the known phrases. */
function phraseKeys(keys: string): string[][] {
  const phrases = ["knock out", "party member", "level cap", "max level", "level limit", "run away"];
  let rest = ` ${keys} `;
  const out: string[][] = [];
  for (const p of phrases) {
    if (rest.includes(` ${p} `)) {
      out.push(p.split(" "));
      rest = rest.replace(` ${p} `, " ");
    }
  }
  for (const w of rest.split(" ").filter(Boolean)) out.push([w]);
  return out;
}

// --------------------------------------------------------------------- index

// Field weights: name, field names, signatures, group, text.
const W = [10, 5, 3.5, 3, 1];
const LABEL = ["Name", "Fields", "Signature", "Group", "Text"];
const ENDINGS = ["", "s", "ed", "ing", "e"];
// Hooks have no description, so they rank below the entries that do.
const SECTION: Record<string, number> = { events: 1.1, functions: 1.05, hooks: 0.7 };
const K1 = 1.2;
const B = 0.4;

interface Doc {
  id: string;
  title: string;
  group: string;
  section: string;
  fields: string[];
  dl: number;
  stemLine: string;
  titleLine: string;
}

export interface SearchIndex {
  docs: Doc[];
  post: Map<string, { d: number[]; w: number[] }>;
  vocab: string[];
  avg: number;
}

type Fields = [string, string, string, string, string];

const join = (...p: (string | null | undefined)[]) => p.filter(Boolean).join(" ");
const lastWord = (p: string) => p.trim().split(/[\s*]+/).pop() ?? "";

function blockText(b: { t: string; [k: string]: unknown }): string {
  if (b.t === "p") return b.text as string;
  if (b.t === "code") return b.text as string;
  if (b.t === "list") return (b.items as string[]).join(" ");
  return [...(b.head as string[]), ...(b.rows as string[][]).flat()].join(" ");
}

export function buildIndex(api: Api, entries: Entry[]): SearchIndex {
  const ev = new Map(api.events.map((e) => [e.id, e]));
  const fn = new Map(api.functions.map((f) => [f.id, f]));
  const hk = new Map(api.hooks.map((h) => [h.id, h]));
  const pg = new Map([...api.pages, api.hookPage].map((p) => [p.id, p]));
  const rc = new Map(api.recipes.map((r) => [r.id, r]));
  const st = new Map(api.state.map((s) => [`state:${s.name}`, s]));
  const recipeTitle = (slug: string) => api.recipes.find((r) => r.slug === slug)?.title;

  const docs: Doc[] = [];
  const post = new Map<string, { d: number[]; w: number[] }>();
  let total = 0;

  for (const e of entries) {
    let f: Fields = [e.title, "", "", e.group, ""];
    const x = ev.get(e.id);
    const y = fn.get(e.id);
    const h = hk.get(e.id);
    const r = rc.get(e.id);
    const s = st.get(e.id);
    const p = pg.get(e.id);
    if (x) {
      f = [
        x.name,
        x.payload.map((q) => join(q.name, q.cName)).join(" "),
        join(x.binding, x.cStruct, x.region),
        x.category,
        join(x.when, x.refuse, x.modifyText, ...x.notes, ...x.caveats, ...x.payload.map((q) => q.meaning), ...x.functions, ...x.recipes.map((z) => recipeTitle(z))),
      ];
    } else if (y) {
      f = [y.name, y.params.map(lastWord).join(" "), y.signature, y.group, join(y.what, y.result, y.rule, ...y.notes)];
    } else if (h) {
      f = [h.name, join(h.key, h.corpusName), h.proto ?? "", e.group, h.events.join(" ")];
    } else if (r) {
      f = [r.title, join(r.slug, ...r.events, ...r.functions), "", e.group, join(r.goal, ...r.steps, ...(r.notes ?? []), r.rule, r.rule2, r.c, r.settings)];
    } else if (s) {
      f = [s.name, "", "", e.group, join(s.value, s.note, s.subscript)];
    } else if (e.id === "page:actions") {
      f = [e.title, api.actions.map((a) => a.name).join(" "), api.actions.map((a) => a.syntax).join(" "), e.group, api.actions.map((a) => `${a.what} ${a.example}`).join(" ")];
    } else if (p) {
      f = [e.title, "", "", e.group, p.blocks.map(blockText).join(" ")];
    }
    const fields = f.map((t) => t.replace(/\s+/g, " ").trim());
    const idx = docs.length;
    const local = new Map<string, number>();
    let dl = 0;
    const lines: string[] = [];
    fields.forEach((t, i) => {
      const ss = stems(t);
      lines.push(ss.join(" "));
      dl += ss.length * W[i];
      for (const q of ss) local.set(q, (local.get(q) ?? 0) + W[i]);
    });
    for (const [q, w] of local) {
      let po = post.get(q);
      if (!po) post.set(q, (po = { d: [], w: [] }));
      po.d.push(idx);
      po.w.push(w);
    }
    total += dl;
    docs.push({
      id: e.id,
      title: e.title,
      group: e.group,
      section: e.section,
      fields,
      dl,
      stemLine: ` ${lines.join(" | ")} `,
      titleLine: ` ${lines[0]} `,
    });
  }
  return { docs, post, vocab: [...post.keys()], avg: total / Math.max(1, docs.length) };
}

// --------------------------------------------------------------------- query

/** Optimal string alignment distance, true when within k. */
function within(a: string, b: string, k: number): boolean {
  const n = a.length;
  const m = b.length;
  if (Math.abs(n - m) > k) return false;
  let p2: number[] = [];
  let p1 = Array.from({ length: m + 1 }, (_, j) => j);
  for (let i = 1; i <= n; i++) {
    const cur = [i];
    let low = i;
    for (let j = 1; j <= m; j++) {
      let v = Math.min(p1[j] + 1, cur[j - 1] + 1, p1[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) v = Math.min(v, p2[j - 2] + 1);
      cur.push(v);
      if (v < low) low = v;
    }
    if (low > k) return false;
    p2 = p1;
    p1 = cur;
  }
  return p1[m] <= k;
}

interface Term {
  // Stems that count, with their weight.
  alts: [string, number][];
  // The typed word of a single-word term, to undo a wrong ending.
  raw: string | null;
  // Raw lower-case word for prefix matching (the last typed word only).
  prefix: string | null;
}

function parse(q: string): Term[] {
  const raw: string[] = [];
  for (const m of q.matchAll(WORD)) for (const p of splitWord(m[0])) raw.push(p);
  let words = raw.filter((w) => !STOP.has(w));
  if (!words.length) words = raw;
  const typing = !/\s$/.test(q) && words.length > 0 && words[words.length - 1] === raw[raw.length - 1];
  const st = words.map(stem);
  const terms: Term[] = [];
  for (let i = 0; i < words.length; ) {
    let used = 1;
    let syn: string[] | undefined;
    for (let len = Math.min(longest, words.length - i); len >= 1; len--) {
      const hit = SYN.get(st.slice(i, i + len).join(" "));
      if (hit) {
        syn = hit;
        used = len;
        break;
      }
    }
    const alts = new Map<string, number>();
    for (let k = i; k < i + used; k++) alts.set(st[k], 1);
    for (const s of syn ?? []) if (!alts.has(s)) alts.set(s, 0.6);
    const last = i + used === words.length;
    terms.push({ alts: [...alts], raw: used === 1 ? words[i] : null, prefix: last && typing ? words[words.length - 1] : null });
    i += used;
  }
  return terms;
}

export interface Part {
  text: string;
  hit: boolean;
}

export interface Hit {
  id: string;
  title: string;
  titleParts: Part[];
  group: string;
  section: string;
  score: number;
  // The field that matched besides the name, and a line from it.
  field: string | null;
  snippet: Part[] | null;
}

interface Run {
  terms: Term[];
  matched: Set<string>;
}

function score(idx: SearchIndex, run: Run, limit: number): { d: number; s: number }[] {
  const { terms } = run;
  const n = idx.docs.length;
  const total = new Float64Array(n);
  const count = new Uint8Array(n);
  const df = (po: { d: number[] }) => Math.log(1 + (n - po.d.length + 0.5) / (po.d.length + 0.5));
  for (const t of terms) {
    const best = new Float64Array(n);
    const add = (stem: string, weight: number) => {
      const po = idx.post.get(stem);
      if (!po) return;
      run.matched.add(stem);
      const idf = df(po);
      for (let i = 0; i < po.d.length; i++) {
        const tf = po.w[i];
        const dl = idx.docs[po.d[i]].dl;
        best[po.d[i]] += (weight * idf * tf * (K1 + 1)) / (tf + K1 * (1 - B + (B * dl) / idx.avg));
      }
    };
    for (const [s, w] of t.alts) {
      if (idx.post.has(s)) add(s, w);
      else {
        // No exact word: the typed prefix first, then a near spelling.
        let found = false;
        if (t.prefix && w === 1) {
          for (const v of idx.vocab) {
            if (v !== s && (v.startsWith(t.prefix) || v.startsWith(s))) {
              add(v, 0.6 * w);
              found = true;
            }
          }
        }
        if (!found && w === 1 && s.length >= 4) {
          const k = s.length >= 8 ? 2 : 1;
          for (const v of idx.vocab) {
            if (within(s, v, k) || (t.raw && ENDINGS.some((e) => within(t.raw!, v + e, k)))) add(v, 0.5);
          }
        }
      }
    }
    // A longer typed word may also be the start of other words.
    if (t.prefix && t.prefix.length >= 2) {
      for (const [s, w] of t.alts) {
        if (w !== 1 || !idx.post.has(s)) continue;
        for (const v of idx.vocab) if (v !== s && v.startsWith(t.prefix)) add(v, 0.5);
      }
    }
    for (let i = 0; i < n; i++) {
      if (best[i] > 0) {
        total[i] += best[i];
        count[i]++;
      }
    }
  }
  const need = terms.length;
  const out: { d: number; s: number }[] = [];
  const phrase = terms.length > 1 && terms.every((t) => t.alts[0][1] === 1) ? ` ${terms.map((t) => t.alts[0][0]).join(" ")} ` : null;
  const finish = (i: number) => {
    const d = idx.docs[i];
    let s = total[i];
    if (phrase && d.stemLine.includes(phrase)) s *= 1.5;
    if (phrase && d.titleLine.includes(phrase)) s += 8;
    const inTitle = terms.filter((t) => t.alts.some(([a]) => d.titleLine.includes(` ${a} `))).length;
    if (inTitle === terms.length) s += 4;
    // A short name that is mostly the query beats a long one that mentions it.
    const words = d.titleLine.trim().split(" ");
    const covered = words.filter((w) => terms.some((t) => t.alts.some(([a, wt]) => wt >= 0.6 && a === w))).length;
    s += (6 * covered) / words.length;
    return s * (SECTION[d.section] ?? 1);
  };
  for (let i = 0; i < n; i++) if (count[i] >= need) out.push({ d: i, s: finish(i) });
  // Few full answers: also offer the ones that miss a single word.
  if (out.length < 5 && need >= 2) {
    for (let i = 0; i < n; i++) if (count[i] === need - 1) out.push({ d: i, s: finish(i) * 0.7 });
  }
  out.sort((a, b) => b.s - a.s || a.d - b.d);
  return out.slice(0, limit);
}

/** True for a word of the text that the matched stems cover. */
function matcher(matched: Set<string>): (word: string) => boolean {
  return (word) => splitWord(word).some((p) => matched.has(stem(p)));
}

function parts(text: string, isHit: (w: string) => boolean): Part[] {
  const out: Part[] = [];
  let at = 0;
  for (const m of text.matchAll(WORD)) {
    if (!isHit(m[0])) continue;
    if (m.index > at) out.push({ text: text.slice(at, m.index), hit: false });
    out.push({ text: m[0], hit: true });
    at = m.index + m[0].length;
  }
  if (at < text.length) out.push({ text: text.slice(at), hit: false });
  return out;
}

function snippetOf(text: string, isHit: (w: string) => boolean): Part[] | null {
  for (const m of text.matchAll(WORD)) {
    if (!isHit(m[0])) continue;
    let from = Math.max(0, m.index - 28);
    if (from > 0) {
      const sp = text.indexOf(" ", from);
      if (sp >= 0 && sp < m.index) from = sp + 1;
    }
    let to = Math.min(text.length, m.index + 80);
    if (to < text.length) {
      const sp = text.lastIndexOf(" ", to);
      if (sp > m.index) to = sp;
    }
    const body = parts(text.slice(from, to), isHit);
    if (from > 0) body.unshift({ text: "... ", hit: false });
    if (to < text.length) body.push({ text: " ...", hit: false });
    return body;
  }
  return null;
}

/** Ranked hits for a query, best first. */
export function searchIndex(idx: SearchIndex, query: string, limit = 50): Hit[] {
  const terms = parse(query);
  if (!terms.length) return [];
  const run: Run = { terms, matched: new Set() };
  const ranked = score(idx, run, limit);
  const isHit = matcher(run.matched);
  return ranked.map(({ d, s }) => {
    const doc = idx.docs[d];
    let field: string | null = null;
    let snippet: Part[] | null = null;
    for (let i = 1; i < 5 && !snippet; i++) {
      snippet = snippetOf(doc.fields[i], isHit);
      if (snippet) field = LABEL[i];
    }
    return { id: doc.id, title: doc.title, titleParts: parts(doc.title, isHit), group: doc.group, section: doc.section, score: s, field, snippet };
  });
}

/** Decides if a word of the page text belongs to the query, for the find bar. */
export function queryMatcher(idx: SearchIndex, query: string): (word: string) => boolean {
  const run: Run = { terms: parse(query), matched: new Set() };
  if (run.terms.length) score(idx, run, 0);
  // Words nobody indexed still count when they equal a query stem.
  for (const t of run.terms) for (const [s] of t.alts) run.matched.add(s);
  return matcher(run.matched);
}
