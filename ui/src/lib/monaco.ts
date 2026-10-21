// What the views share with the code editor without loading Monaco: the
// language names, the problem shape, and the names the editor completes.
// Monaco itself (monacoSetup.ts) loads when the first editor opens.
import type { RuleNames } from "./modApi";

export const RULE_LANGUAGE = "eidos-rule";
export const HOOK_LANGUAGE = "eidos-hooks";
export const TABLE_LANGUAGE = "eidos-table";

/// A problem to draw in the editor (lines and columns start at 1).
export interface Marker {
  line: number;
  start: number;
  end: number;
  severity: "error" | "warning";
  message: string;
}

/// `[region, address, function, events]` per hookable function.
export type DeclaredRow = [string, string, string, string[]];

export const completions: { ruleNames: RuleNames | null; declared: DeclaredRow[]; tables: string[] } = {
  ruleNames: null,
  declared: [],
  tables: [],
};

/// The names the rule language completes (events, payload fields, state).
export function setRuleNames(names: RuleNames) {
  completions.ruleNames = names;
}

/// The mod's table names, which `table.` completes.
export function setTables(names: string[]) {
  completions.tables = names;
}

/// The hookable functions the hook lines complete.
export function setDeclared(rows: DeclaredRow[]) {
  completions.declared = rows;
}

export const loadMonaco = () => import("./monacoSetup");
