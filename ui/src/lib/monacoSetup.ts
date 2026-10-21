// Monaco, loaded from the app's own files (no CDN), with the two languages
// kenon edits as text: the rule language and the hook lines of the manifest.
// The editor is created in CodeEditor.svelte. This file is loaded on demand
// through loadMonaco in monaco.ts, so the editor costs nothing until used.
import * as monaco from "monaco-editor/editor/editor.api";
import EditorWorker from "monaco-editor/editor/editor.worker?worker";
// The editing features the standalone build wires in; the language packs
// and their services are left out.
import "monaco-editor/editor/contrib/bracketMatching/browser/bracketMatching";
import "monaco-editor/editor/contrib/clipboard/browser/clipboard";
import "monaco-editor/editor/contrib/comment/browser/comment";
import "monaco-editor/editor/contrib/contextmenu/browser/contextmenu";
import "monaco-editor/editor/contrib/cursorUndo/browser/cursorUndo";
import "monaco-editor/editor/contrib/find/browser/findController";
import "monaco-editor/editor/contrib/folding/browser/folding";
import "monaco-editor/editor/contrib/gotoError/browser/gotoError";
import "monaco-editor/editor/contrib/hover/browser/hoverContribution";
import "monaco-editor/editor/contrib/indentation/browser/indentation";
import "monaco-editor/editor/contrib/linesOperations/browser/linesOperations";
import "monaco-editor/editor/contrib/multicursor/browser/multicursor";
import "monaco-editor/editor/contrib/readOnlyMessage/browser/contribution";
import "monaco-editor/editor/contrib/smartSelect/browser/smartSelect";
import "monaco-editor/editor/contrib/suggest/browser/suggestController";
import "monaco-editor/editor/contrib/wordHighlighter/browser/wordHighlighter";
import "monaco-editor/editor/contrib/wordOperations/browser/wordOperations";
import "monaco-editor/editor/browser/coreCommands";
import "monaco-editor/editor/common/standaloneStrings";
import "../../../node_modules/monaco-editor/esm/vs/base/browser/ui/codicons/codicon/codicon.css";
import { HOOK_LANGUAGE, RULE_LANGUAGE, completions } from "./monaco";

export { monaco };

let ready = false;

const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
const hex = (name: string, fallback = "#808080") => {
  const v = css(name);
  return /^#[0-9a-f]{6}$/i.test(v) ? v.slice(1) : fallback.slice(1);
};

/// Defines the editor theme from kenon's tokens and switches to it.
function applyTheme() {
  const dark = document.documentElement.dataset.theme !== "light";
  const name = dark ? "kenon-dark" : "kenon-light";
  const c = (v: string) => `#${hex(v)}`;
  monaco.editor.defineTheme(name, {
    base: dark ? "vs-dark" : "vs",
    inherit: true,
    rules: [
      { token: "comment", foreground: hex("--faint"), fontStyle: "italic" },
      { token: "keyword", foreground: hex("--accent"), fontStyle: "bold" },
      { token: "keyword.action", foreground: hex("--gold"), fontStyle: "bold" },
      { token: "string", foreground: hex("--ok") },
      { token: "number", foreground: hex("--gold") },
      { token: "number.hex", foreground: hex("--gold") },
      { token: "type", foreground: hex("--accent") },
      { token: "variable.predefined", foreground: hex("--muted"), fontStyle: "bold" },
      { token: "variable.state", foreground: hex("--gold") },
      { token: "constant", foreground: hex("--gold") },
      { token: "attribute.name", foreground: hex("--gold") },
      { token: "delimiter", foreground: hex("--muted") },
      { token: "operator", foreground: hex("--muted") },
    ],
    colors: {
      "editor.background": c("--bg"),
      "editor.foreground": c("--ink"),
      "editorLineNumber.foreground": c("--faint"),
      "editorLineNumber.activeForeground": c("--muted"),
      "editor.lineHighlightBackground": c("--panel-2"),
      "editor.selectionBackground": c("--accent") + "55",
      "editor.inactiveSelectionBackground": c("--accent") + "33",
      "editorCursor.foreground": c("--ink"),
      "editorIndentGuide.background1": c("--rim"),
      "editorWidget.background": c("--panel"),
      "editorWidget.border": c("--rim"),
      "editorSuggestWidget.background": c("--panel"),
      "editorSuggestWidget.selectedBackground": c("--panel-3"),
      "editorHoverWidget.background": c("--panel"),
      "editorError.foreground": c("--bad"),
      "editorWarning.foreground": c("--gold"),
      focusBorder: c("--gold"),
    },
  });
  monaco.editor.setTheme(name);
}

const ACTIONS = ["refuse", "set", "add", "sub", "message", "send"];

function registerRule() {
  monaco.languages.register({ id: RULE_LANGUAGE, extensions: [".rule"] });
  monaco.languages.setLanguageConfiguration(RULE_LANGUAGE, {
    comments: { lineComment: "#" },
    brackets: [
      ["(", ")"],
      ["[", "]"],
    ],
    autoClosingPairs: [
      { open: "(", close: ")" },
      { open: "[", close: "]" },
      { open: '"', close: '"', notIn: ["string"] },
    ],
    indentationRules: {
      increaseIndentPattern: /^\s*rule\b/,
      decreaseIndentPattern: /^\s*$^/,
    },
  });
  monaco.languages.setMonarchTokensProvider(RULE_LANGUAGE, {
    defaultToken: "",
    keywords: ["language", "rule", "on", "when", "do"],
    actions: ACTIONS,
    states: [
      "money",
      "badges",
      "badge",
      "zone",
      "party_count",
      "species",
      "level",
      "hp",
      "max_hp",
      "type1",
      "type2",
      "shiny",
      "held_item",
      "species_type1",
      "species_type2",
      "species_bst",
      "caught",
      "item_count",
      "battle_style",
      "in_battle",
    ],
    tokenizer: {
      root: [
        [/#.*$/, "comment"],
        [/(\brule\b)(\s+)([\w.-]+)(\s+)(\bon\b)(\s+)([\w.]+)/, ["keyword", "", "identifier", "", "keyword", "", "type"]],
        [/"(?:[^"\\]|\\.)*"/, "string"],
        [/"/, "string.invalid"],
        [/\d+/, "number"],
        [/\b(?:true|false)\b/, "constant"],
        [/\b(?:event|state)\b(?=\.)/, "variable.predefined"],
        [
          /[A-Za-z_]\w*/,
          { cases: { "@keywords": "keyword", "@actions": "keyword.action", "@states": "variable.state", "@default": "identifier" } },
        ],
        [/[-+*/%<>=!&|]+/, "operator"],
        [/[()[\].]/, "delimiter"],
      ],
    },
  } as monaco.languages.IMonarchLanguage);

  monaco.languages.registerCompletionItemProvider(RULE_LANGUAGE, {
    triggerCharacters: [".", " "],
    provideCompletionItems(model, position) {
      const names = completions.ruleNames;
      if (!names) return { suggestions: [] };
      const before = model.getLineContent(position.lineNumber).slice(0, position.column - 1);
      const word = model.getWordUntilPosition(position);
      const range = new monaco.Range(position.lineNumber, word.startColumn, position.lineNumber, word.endColumn);
      const K = monaco.languages.CompletionItemKind;
      const make = (label: string, kind: monaco.languages.CompletionItemKind, detail = "", insertText = label) => ({
        label,
        kind,
        detail,
        insertText,
        range,
      });
      // After "on": the events.
      if (/^\s*rule\s+\S+\s+on\s+[\w.]*$/.test(before)) {
        return {
          suggestions: names.events.map((e) => make(e.name, K.Event, e.refusable ? "can be refused" : "")),
        };
      }
      // The event of the file, from its "rule ... on" line.
      const header = /^\s*rule\s+\S+\s+on\s+([\w.]+)/m.exec(model.getValue());
      const event = names.events.find((e) => e.name === header?.[1]);
      if (/\bevent\.\w*$/.test(before)) {
        const target = /^\s*do\s+(?:set|add|sub)\s+event\.\w*$/.test(before);
        return {
          suggestions: (event?.fields ?? [])
            .filter((f) => (target ? f.writable : f.kind !== "list"))
            .map((f) => make(f.name, K.Field, `${f.kind}${f.writable ? ", a rule can write it" : ""}`)),
        };
      }
      if (/^\s*do\s+\w*$/.test(before)) {
        return {
          suggestions: ACTIONS.filter((a) => a !== "refuse" || event?.refusable !== false).map((a) =>
            make(a, K.Keyword),
          ),
        };
      }
      if (/^\s*\w*$/.test(before)) {
        return {
          suggestions: [
            make("language", K.Keyword, "", "language 1"),
            make("rule", K.Keyword, "", "rule ${1:name} on ${2:event}"),
            make("when", K.Keyword),
            make("do", K.Keyword),
          ].map((s) => ({ ...s, insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet })),
        };
      }
      const writing = /^\s*do\s+(?:set|add|sub)\s+\w*$/.test(before);
      return {
        suggestions: [
          make("event", K.Module, "payload of the event", "event."),
          make("state", K.Module, "the mod's own cell", "state."),
          ...names.states
            .filter((s) => (writing ? s.writable && s.writableOk : s.readable))
            .map((s) =>
              make(s.name, K.Variable, s.sub === "plain" ? "" : s.sub === "mon" ? "takes a Pokemon" : "takes a number"),
            ),
          ...(writing ? [] : [make("true", K.Constant), make("false", K.Constant)]),
        ],
      };
    },
  });
}

function registerHooks() {
  monaco.languages.register({ id: HOOK_LANGUAGE });
  monaco.languages.setLanguageConfiguration(HOOK_LANGUAGE, {
    comments: { lineComment: "#" },
    autoClosingPairs: [{ open: '"', close: '"' }],
  });
  monaco.languages.setMonarchTokensProvider(HOOK_LANGUAGE, {
    defaultToken: "",
    tokenizer: {
      root: [
        [/#.*$/, "comment"],
        [/(\barm9|\bov\d+)(:)(0[xX][0-9a-fA-F]+)/, ["type", "delimiter", "number.hex"]],
        [/\b(?:wrap|replace)\b/, "keyword"],
        [/\b(?:sig|switch)(?==)/, "attribute.name"],
        [/"[^"]*"?/, "string"],
        [/=/, "delimiter"],
        [/\w+/, "identifier"],
      ],
    },
  } as monaco.languages.IMonarchLanguage);
  monaco.languages.registerCompletionItemProvider(HOOK_LANGUAGE, {
    triggerCharacters: [" ", ":"],
    provideCompletionItems(model, position) {
      const before = model.getLineContent(position.lineNumber).slice(0, position.column - 1);
      const word = model.getWordUntilPosition(position);
      const K = monaco.languages.CompletionItemKind;
      const at = (from: number) => new monaco.Range(position.lineNumber, from, position.lineNumber, position.column);
      if (/^\s*\S*$/.test(before)) {
        const from = position.column - (/^\s*(\S*)$/.exec(before)?.[1].length ?? 0);
        return {
          suggestions: completions.declared.map(([region, addr, fn, events]) => ({
            label: `${region}:${addr}`,
            kind: K.Function,
            detail: fn,
            documentation: events.length ? `Events: ${events.join(", ")}` : undefined,
            insertText: `${region}:${addr} `,
            filterText: `${region}:${addr} ${fn}`,
            range: at(from),
          })),
        };
      }
      const r = new monaco.Range(position.lineNumber, word.startColumn, position.lineNumber, word.endColumn);
      if (/^\s*\S+\s+\w*$/.test(before)) {
        return {
          suggestions: ["wrap", "replace"].map((m) => ({
            label: m,
            kind: K.Keyword,
            insertText: m + " ",
            range: r,
          })),
        };
      }
      if (/^\s*\S+\s+\S+\s+\S+\s+[\w=]*$/.test(before)) {
        return {
          suggestions: [
            { label: "sig", kind: K.Property, insertText: 'sig="${1:int (int)}"', range: r },
            { label: "switch", kind: K.Property, insertText: "switch=${1:name}", range: r },
          ].map((s) => ({ ...s, insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet })),
        };
      }
      return { suggestions: [] };
    },
  });
  monaco.languages.registerHoverProvider(HOOK_LANGUAGE, {
    provideHover(model, position) {
      const m = /(arm9|ov\d+):(0[xX][0-9a-fA-F]+)/.exec(model.getLineContent(position.lineNumber));
      if (!m) return null;
      const addr = parseInt(m[2], 16) & ~1;
      const row = completions.declared.find(([r, a]) => r === m[1].replace(/^ov0+(\d)/, "ov$1") && parseInt(a, 16) === addr);
      if (!row) return null;
      return {
        contents: [{ value: `**${row[2]}**` }, ...(row[3].length ? [{ value: `Events: ${row[3].join(", ")}` }] : [])],
      };
    },
  });
}

/// Sets Monaco up once: workers, languages and the theme.
export function ensureMonaco() {
  if (ready) return;
  ready = true;
  (self as unknown as { MonacoEnvironment: monaco.Environment }).MonacoEnvironment = {
    getWorker: () => new EditorWorker(),
  };
  registerRule();
  registerHooks();
  applyTheme();
  new MutationObserver(applyTheme).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
  });
}
