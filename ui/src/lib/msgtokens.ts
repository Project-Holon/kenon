// Message text with readable tokens. The game's text holds control codes;
// the backend shows them as `{KKKK:a,b}` (kind in hex, arguments in hex).
// For editing they read as `[Player]`, `[Item 1]`, `[Wait]`, and turn back
// into the exact codes on save. A code this table does not know stays as it
// is, so every string round-trips.
//
// A code is a 0xF000 unit, then group << 8 | kind, a count, the arguments
// (aletheia arm9/printsys.c: PRINTSYS_AppendTag, PRINTSYS_GetTagGroup).
//
//   group 0x01, 0x02  a word slot: the argument is the slot the script
//                     filled (WORDSET_CopyTaggedText, PRINTSYS_IsWordSlotTag).
//                     The game reads only the group and the slot; the kind
//                     says what the word is, taken from where the messages
//                     use it ("{0100:0} used the {0109:1}": name, item).
//   group 0xFF        text colour: argument = palette row, 0xFF = the colour
//                     the last 0xBD00 set (PRINTSYS_ProcColorTag)
//   group 0xBD        layout (PRINTSYS_ProcGeneralTag): 0 colours, 1 default
//                     colours, 2 centre, 3 right align, 4 shift, 5 go to x
//   group 0xBE        pauses and cues (PRINTSYS_Apply): 0 wait and scroll,
//                     1 wait and clear, 2 pause frames, 3 delay per letter,
//                     4 delay reset, 5 and 6 cue, 7 cue reset, 8 clear
//                     window, 9 speed
//
// Escapes of the display form stay: `\n`, `\\`, `\{`, `\}`. A literal `[` or
// `]` in the text is written `\[` and `\]` while editing.

interface Def {
  kind: number;
  args: number;
  label: string;
  /// Arguments the label stands for, when it has no number of its own.
  fixed?: number[];
}

const slot = (kind: number, label: string): Def => ({ kind, args: 1, label });
const plain = (kind: number, label: string, args = 0): Def => ({ kind, args, label });

const DEFS: Def[] = [
  { kind: 0x100, args: 0, label: "Player", fixed: [0] },
  slot(0x100, "Name"),
  slot(0x101, "Species"),
  slot(0x102, "Nickname"),
  slot(0x103, "Type"),
  slot(0x105, "Place"),
  slot(0x106, "Ability"),
  slot(0x107, "Move"),
  slot(0x108, "Nature"),
  slot(0x109, "Item"),
  slot(0x10a, "Prop"),
  slot(0x10b, "Box"),
  slot(0x10c, "Pokemon"),
  slot(0x10d, "Stat"),
  slot(0x10e, "Trainer class"),
  slot(0x110, "Power name"),
  slot(0x111, "Pass Power"),
  slot(0x112, "Pocket"),
  slot(0x113, "Answer"),
  slot(0x11c, "Phrase"),
  slot(0x122, "Show"),
  slot(0x123, "Style"),
  slot(0x124, "Country"),
  slot(0x125, "Region"),
  slot(0x128, "Interest"),
  slot(0x131, "Body part"),
  slot(0x132, "Decor"),
  slot(0x180, "Month"),
  slot(0x181, "Rank"),
  slot(0x200, "Number"),
  ...Array.from({ length: 8 }, (_, i) => slot(0x201 + i, `${i + 2}-digit number`)),
  { kind: 0xff00, args: 0, label: "Normal", fixed: [0] },
  { kind: 0xff00, args: 0, label: "Color end", fixed: [0xff] },
  plain(0xff00, "Color", 1),
  plain(0xbd00, "Text colors", 3),
  plain(0xbd01, "Default colors"),
  plain(0xbd02, "Center"),
  plain(0xbd03, "Right align", 1),
  plain(0xbd04, "Shift", 1),
  plain(0xbd05, "Move to", 1),
  plain(0xbe00, "Wait scroll"),
  plain(0xbe01, "Wait"),
  plain(0xbe02, "Pause", 1),
  plain(0xbe03, "Letter delay", 1),
  plain(0xbe04, "Letter delay reset"),
  plain(0xbe05, "Cue", 1),
  plain(0xbe06, "Next cue", 1),
  plain(0xbe07, "Cue reset"),
  plain(0xbe08, "Clear window"),
  plain(0xbe09, "Speed", 1),
];

const byLabel = new Map(DEFS.map((d) => [d.label, d]));
// The labels that stand for fixed arguments win over the plain kind label.
const byCode = new Map<string, string>();
for (const d of DEFS) byCode.set(`${d.kind}:${(d.fixed ?? []).join(",")}`, d.label);

const hex = (n: number) => n.toString(16).toUpperCase();

/// The token for the body of a display-form code (`0100:0`), or null.
function tokenFor(body: string): string | null {
  const m = /^([0-9A-F]{4})(?::([0-9A-F]+(?:,[0-9A-F]+)*))?$/.exec(body);
  if (!m) return null;
  const kind = parseInt(m[1], 16);
  const args = m[2] ? m[2].split(",") : [];
  // Only the form the backend writes: no leading zeros, upper case.
  const nums = args.map((a) => parseInt(a, 16));
  if (nums.some((n, i) => hex(n) !== args[i] || n > 0xffff)) return null;
  const fixed = byCode.get(`${kind}:${nums.join(",")}`);
  if (fixed !== undefined && byLabel.get(fixed)!.fixed) return `[${fixed}]`;
  const label = byCode.get(`${kind}:`);
  const def = label === undefined ? undefined : byLabel.get(label);
  if (!def || def.fixed || def.args !== nums.length) return null;
  return nums.length ? `[${def.label} ${nums.join(" ")}]` : `[${def.label}]`;
}

/// The display-form code for the inside of a token (`Item 1`), or null.
function codeFor(body: string): string | null {
  const words = body.split(" ");
  for (let k = words.length; k >= 1; k--) {
    const def = byLabel.get(words.slice(0, k).join(" "));
    if (!def) continue;
    const rest = words.slice(k);
    const args = def.fixed ?? rest.map(Number);
    if (!def.fixed && (rest.length !== def.args || rest.some((w) => !/^\d{1,5}$/.test(w)))) continue;
    if (def.fixed && rest.length) continue;
    if (args.some((n) => n > 0xffff)) continue;
    const kind = hex(def.kind).padStart(4, "0");
    return args.length ? `{${kind}:${args.map(hex).join(",")}}` : `{${kind}}`;
  }
  return null;
}

/// Display form (backend) to editing form.
export function toTokens(display: string): string {
  let out = "";
  let i = 0;
  while (i < display.length) {
    const c = display[i];
    if (c === "\\") {
      out += display.slice(i, i + 2);
      i += 2;
    } else if (c === "{") {
      const j = display.indexOf("}", i);
      if (j < 0) {
        out += display.slice(i);
        break;
      }
      out += tokenFor(display.slice(i + 1, j)) ?? display.slice(i, j + 1);
      i = j + 1;
    } else if (c === "[" || c === "]") {
      out += `\\${c}`;
      i++;
    } else {
      out += c;
      i++;
    }
  }
  return out;
}

/// Editing form back to display form. Raw `{...}` codes pass through, and
/// brackets that name no token stay text.
export function fromTokens(text: string): string {
  let out = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === "\\") {
      const n = text[i + 1];
      out += n === "[" || n === "]" ? n : text.slice(i, i + 2);
      i += 2;
    } else if (c === "{") {
      const j = text.indexOf("}", i);
      if (j < 0) {
        out += text.slice(i);
        break;
      }
      out += text.slice(i, j + 1);
      i = j + 1;
    } else if (c === "[") {
      const j = text.indexOf("]", i);
      const code = j < 0 ? null : codeFor(text.slice(i + 1, j));
      if (code !== null) {
        out += code;
        i = j + 1;
      } else {
        out += c;
        i++;
      }
    } else {
      out += c;
      i++;
    }
  }
  return out;
}

/// Text for a multi-line editor: tokens, and real line breaks for `\n`.
export function toEditText(display: string): string {
  return toTokens(display).replace(/\\(.)/g, (m, c) => (c === "n" ? "\n" : m));
}

/// The display form of text from a multi-line editor.
export function fromEditText(text: string): string {
  return fromTokens(text.replace(/\r?\n/g, "\\n"));
}

/// Every token with the code it stands for, for help text and checks.
export function tokenList(): { token: string; code: string }[] {
  return DEFS.map((d) => {
    const body = d.fixed || d.args === 0 ? d.label : `${d.label} ${Array(d.args).fill(1).join(" ")}`;
    return { token: `[${body}]`, code: codeFor(body) ?? "" };
  });
}
