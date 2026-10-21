// Checks that every string of the text banks survives the token form.
//
//   KENON_TEXT_DUMP=out.json KENON_ROM=game.nds \
//     cargo test -p kenon-core --test user_data dump_text_for_tokens
//   node tools/check-msgtokens.mjs out.json
//
// For each string: display -> tokens -> display must give the same string,
// and the same through the multi-line editor form. It also counts the codes
// that became tokens and lists the kinds that stayed raw.
import { readFileSync } from "node:fs";
import { fromEditText, fromTokens, toEditText, toTokens, tokenList } from "../ui/src/lib/msgtokens.ts";

const file = process.argv[2] ?? process.env.KENON_TEXT_DUMP;
if (!file) {
  console.error("usage: node tools/check-msgtokens.mjs dump.json");
  process.exit(2);
}
const banks = JSON.parse(readFileSync(file, "utf8"));

// Every token in the help list maps to a code and back.
for (const { token, code } of tokenList()) {
  if (!code || toTokens(code) !== token) {
    console.error(`token table: ${token} -> ${code} -> ${toTokens(code)}`);
    process.exit(1);
  }
}

// The `{...}` codes of a display string, skipping escaped braces.
function* codesOf(s) {
  for (let i = 0; i < s.length; i++) {
    if (s[i] === "\\") i++;
    else if (s[i] === "{") {
      const j = s.indexOf("}", i);
      if (j < 0) return;
      if (/^{[0-9A-F]{4}(:[0-9A-F,]+)?}$/.test(s.slice(i, j + 1))) yield s.slice(i, j + 1);
      i = j;
    }
  }
}

let strings = 0;
let codes = 0;
let tokens = 0;
const raw = new Map();
const bad = [];
for (const [b, bank] of banks.entries()) {
  for (const [i, s] of bank.entries()) {
    strings++;
    const t = toTokens(s);
    if (fromTokens(t) !== s || fromEditText(toEditText(s)) !== s) bad.push([b, i, s]);
    for (const code of codesOf(s)) {
      codes++;
      if (toTokens(code).startsWith("{")) raw.set(code.slice(1, 5), (raw.get(code.slice(1, 5)) ?? 0) + 1);
      else tokens++;
    }
  }
}
console.log(`${strings} strings, ${codes} codes, ${tokens} became tokens, ${codes - tokens} stayed raw`);
if (raw.size) console.log("raw kinds:", [...raw].map(([k, n]) => `${k} x${n}`).join(", "));
if (bad.length) {
  console.error(`${bad.length} strings do not round-trip, first:`, bad.slice(0, 3));
  process.exit(1);
}
console.log("every string round-trips");
