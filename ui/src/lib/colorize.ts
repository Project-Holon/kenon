// Colours a piece of code as the editor would, for read-only views such as the
// mod docs. Uses the editor's own languages and theme.
import { HOOK_LANGUAGE, RULE_LANGUAGE, TABLE_LANGUAGE, loadMonaco } from "./monaco";
import { C_LANGUAGE, ensureC } from "./monacoC";

export type CodeKind = "rule" | "hooks" | "table" | "c" | null;

const LANGUAGE: Record<Exclude<CodeKind, null>, string> = {
  rule: RULE_LANGUAGE,
  hooks: HOOK_LANGUAGE,
  table: TABLE_LANGUAGE,
  c: C_LANGUAGE,
};

/// The coloured HTML for `text`, or null when the kind has no colours.
export async function colorize(text: string, kind: CodeKind): Promise<string | null> {
  if (!kind) return null;
  const setup = await loadMonaco();
  setup.ensureMonaco();
  if (kind === "c") await ensureC();
  return setup.monaco.editor.colorize(text, LANGUAGE[kind], { tabSize: 2 });
}

/// A Svelte action: shows `text` plain at once, then coloured. `done` runs
/// after the coloured version replaced the plain one.
export function colored(node: HTMLElement, arg: { text: string; kind: CodeKind; done?: () => void }) {
  let run = 0;
  const show = (a: typeof arg) => {
    const my = ++run;
    node.textContent = a.text;
    colorize(a.text, a.kind)
      .then((html) => {
        if (my !== run || html === null) return;
        node.innerHTML = html;
        a.done?.();
      })
      .catch(() => {});
  };
  show(arg);
  return {
    update: show,
    destroy: () => {
      run++;
    },
  };
}
