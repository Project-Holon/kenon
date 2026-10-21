// Typed wrappers for the mod code commands in app/src/lib.rs
// (core/src/modcode.rs).
import { invoke } from "@tauri-apps/api/core";

export interface Diagnostic {
  /// 1-based; 0 when the problem is with a form field or the whole file.
  line: number;
  start: number;
  end: number;
  severity: "error" | "warning";
  message: string;
  field: string | null;
}

export interface ModSettings {
  name: string;
  order: number;
  api: number | null;
  port: string | null;
  code: string | null;
  state: string | null;
  settings: string | null;
}

export interface HookCheckRow {
  line: number;
  function: string;
  events: string[];
}

export interface HooksCheck {
  diagnostics: Diagnostic[];
  rows: HookCheckRow[];
}

export interface RuleEntry {
  path: string;
  bound: string | null;
  fileEvent: string | null;
  exists: boolean;
}

export interface RuleField {
  name: string;
  kind: "int" | "mon" | "list";
  writable: boolean;
  signed: boolean;
}

export interface RuleEvent {
  name: string;
  refusable: boolean;
  fields: RuleField[];
}

export interface StateField {
  name: string;
  sub: "plain" | "mon" | "int";
  writable: boolean;
  readable: boolean;
  writableOk: boolean;
}

export interface RuleNames {
  events: RuleEvent[];
  states: StateField[];
}

export interface ModRef {
  name: string;
  dir: string;
  order: number;
  this: boolean;
}

export interface Conflict {
  kind: "file" | "hook";
  key: string;
  mods: ModRef[];
  winner: string;
  detail: string[];
}

export interface ConflictReport {
  mods: ModRef[];
  conflicts: Conflict[];
}

export interface ModPaths {
  modsFolder: string | null;
  declaredSet: string | null;
  declaredFound: number | null;
}

/// The form, how many hook lines the mod has, and the api the port accepts.
export const modGetSettings = () => invoke<[ModSettings, number, number]>("mod_get_settings");
export const modCheckSettings = (settings: ModSettings) => invoke<Diagnostic[]>("mod_check_settings", { settings });
export const modSaveSettings = (settings: ModSettings) => invoke<void>("mod_save_settings", { settings });
export const modGetHooks = () => invoke<string>("mod_get_hooks");
export const modCheckHooks = (text: string) => invoke<HooksCheck>("mod_check_hooks", { text });
export const modSaveHooks = (text: string) => invoke<void>("mod_save_hooks", { text });
/// `[region, address, function, events]` per hookable function.
export const modDeclared = () => invoke<[string, string, string, string[]][]>("mod_declared");
export const modRuleNames = () => invoke<RuleNames>("mod_rule_names");
export const modListRules = () => invoke<RuleEntry[]>("mod_list_rules");
export const modReadRule = (path: string) => invoke<string>("mod_read_rule", { path });
export const modCheckRule = (text: string) => invoke<Diagnostic[]>("mod_check_rule", { text, event: null });
export const modSaveRule = (path: string, text: string) => invoke<Diagnostic[]>("mod_save_rule", { path, text });
export const modNewRule = (name: string, event: string) => invoke<string>("mod_new_rule", { name, event });
export const modRenameRule = (from: string, to: string) => invoke<string>("mod_rename_rule", { from, to });
export const modDeleteRule = (path: string) => invoke<void>("mod_delete_rule", { path });
export const modGetPaths = () => invoke<ModPaths>("mod_get_paths");
export const modSetPaths = (modsFolder: string | null, declaredSet: string | null) =>
  invoke<ModPaths>("mod_set_paths", { modsFolder, declaredSet });
export const modConflicts = () => invoke<ConflictReport>("mod_conflicts");
