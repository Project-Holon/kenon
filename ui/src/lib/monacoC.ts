// The C language for the code editor: colours, and completion and hover from
// the mod's eidos_wasm.h (core/src/codemod.rs, parse_header). Monaco's own C
// pack is not bundled, so this is a small tokenizer of our own. Registered
// once, when the first code editor opens.
import { loadMonaco } from "./monaco";
import type { CodeApi } from "./modApi";
import { completionDocs, hoverFor, loadApi, type Api } from "./moddocs";

export const C_LANGUAGE = "c";

const KEYWORDS = [
  "break", "case", "const", "continue", "default", "do", "else", "enum", "extern", "for", "goto", "if",
  "inline", "return", "sizeof", "static", "struct", "switch", "typedef", "union", "volatile", "while",
];
const TYPES = [
  "void", "char", "short", "int", "long", "float", "double", "signed", "unsigned", "size_t", "bool",
  "int8_t", "int16_t", "int32_t", "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "uintptr_t",
];

let api: CodeApi = { functions: [], constants: [] };
let registered = false;
// The mod docs: events, services and constants explained in the editor.
let docs: Api | null = null;

/// The names to complete, from the mod's header.
export function setCodeApi(next: CodeApi) {
  api = next;
}

export async function ensureC() {
  if (registered) return;
  registered = true;
  const { monaco } = await loadMonaco();
  loadApi()
    .then((a) => (docs = a))
    .catch(() => {});
  const K = monaco.languages.CompletionItemKind;
  monaco.languages.register({ id: C_LANGUAGE, extensions: [".c", ".h"] });
  monaco.languages.setLanguageConfiguration(C_LANGUAGE, {
    comments: { lineComment: "//", blockComment: ["/*", "*/"] },
    brackets: [
      ["{", "}"],
      ["[", "]"],
      ["(", ")"],
    ],
    autoClosingPairs: [
      { open: "{", close: "}" },
      { open: "[", close: "]" },
      { open: "(", close: ")" },
      { open: '"', close: '"', notIn: ["string"] },
      { open: "'", close: "'", notIn: ["string", "comment"] },
    ],
    indentationRules: {
      increaseIndentPattern: /^.*\{[^}"']*$/,
      decreaseIndentPattern: /^\s*\}/,
    },
  });
  monaco.languages.setMonarchTokensProvider(C_LANGUAGE, {
    defaultToken: "",
    keywords: KEYWORDS,
    types: TYPES,
    tokenizer: {
      root: [
        [/^\s*#\s*\w+/, "keyword.directive"],
        [/\/\/.*$/, "comment"],
        [/\/\*/, "comment", "@comment"],
        [/"(?:[^"\\]|\\.)*"/, "string"],
        [/'(?:[^'\\]|\\.)+'/, "string"],
        [/0[xX][0-9a-fA-F]+[uUlL]*/, "number.hex"],
        [/\d+[uUlL]*/, "number"],
        [/\b(?:NULL|true|false)\b/, "constant"],
        [/\bEIDOS_[A-Z0-9_]+\b/, "constant"],
        [
          /[A-Za-z_]\w*/,
          { cases: { "@keywords": "keyword", "@types": "type", "@default": "identifier" } },
        ],
        [/[-+*/%<>=!&|^~?:]+/, "operator"],
        [/[{}()[\];,.]/, "delimiter"],
      ],
      comment: [
        [/[^/*]+/, "comment"],
        [/\*\//, "comment", "@pop"],
        [/[/*]/, "comment"],
      ],
    },
  } as import("monaco-editor/editor/editor.api").languages.IMonarchLanguage);

  monaco.languages.registerCompletionItemProvider(C_LANGUAGE, {
    provideCompletionItems(model, position) {
      const word = model.getWordUntilPosition(position);
      const range = new monaco.Range(position.lineNumber, word.startColumn, position.lineNumber, word.endColumn);
      return {
        suggestions: [
          ...api.functions.map((f) => ({
            label: f.name,
            kind: K.Function,
            detail: f.signature,
            documentation: f.doc || undefined,
            insertText: f.name,
            range,
          })),
          ...api.constants.map((c) => ({
            label: c.name,
            kind: K.Constant,
            detail: c.value,
            documentation: c.group || undefined,
            insertText: c.name,
            range,
          })),
          ...(docs ? completionDocs(docs) : [])
            .filter((d) => !api.functions.some((f) => f.name === d.name))
            .map((d) => ({
              label: d.name,
              kind: K.Function,
              detail: d.detail,
              documentation: d.doc,
              insertText: d.name,
              range,
            })),
          ...KEYWORDS.map((k) => ({ label: k, kind: K.Keyword, insertText: k, range })),
          ...TYPES.map((t) => ({ label: t, kind: K.Class, insertText: t, range })),
        ],
      };
    },
  });

  monaco.languages.registerHoverProvider(C_LANGUAGE, {
    provideHover(model, position) {
      const word = model.getWordAtPosition(position)?.word;
      if (!word) return null;
      const note = docs ? hoverFor(docs, word) : null;
      const extra = note ? [{ value: note }] : [];
      const f = api.functions.find((x) => x.name === word);
      if (f) {
        return { contents: [{ value: "```c\n" + f.signature + "\n```" }, ...(f.doc ? [{ value: f.doc }] : []), ...extra] };
      }
      const c = api.constants.find((x) => x.name === word);
      if (c) return { contents: [{ value: `${c.name} = ${c.value}${c.group ? ` (${c.group})` : ""}` }, ...extra] };
      return extra.length ? { contents: extra } : null;
    },
  });
}
