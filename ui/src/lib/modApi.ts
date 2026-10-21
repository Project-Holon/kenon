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
  tables: TableDecl[];
}

/// One `table = <name> <path>` line of the manifest.
export interface TableDecl {
  name: string;
  path: string;
}

export interface TableEntry {
  name: string;
  path: string;
  exists: boolean;
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
/// One option of the mod's options file. `line`, `lead`, `comment` and `raw`
/// keep the file's comments and unedited lines; send them back unchanged.
export interface OptionDecl {
  key: string;
  label: string;
  kind: "bool" | "choice" | "int";
  /// 0 or 1 for a bool, 0 for a choice (the first value), the number for an int.
  default: number;
  min: number;
  max: number;
  values: string[];
  line: number;
  lead: string[];
  comment: string;
  raw: string;
}

export interface OptionsFile {
  path: string | null;
  exists: boolean;
  options: OptionDecl[];
  diagnostics: Diagnostic[];
}

export const modReadOptions = () => invoke<OptionsFile>("mod_read_options");
export const modSaveOptions = (options: OptionDecl[]) => invoke<OptionsFile>("mod_save_options", { options });
/// A problem of option `n` has the field `option:n`.
export const modCheckOptions = (options: OptionDecl[]) => invoke<Diagnostic[]>("mod_check_options", { options });
export const modSaveRule = (path: string, text: string) => invoke<Diagnostic[]>("mod_save_rule", { path, text });
export const modNewRule = (name: string, event: string) => invoke<string>("mod_new_rule", { name, event });
export const modRenameRule = (from: string, to: string) => invoke<string>("mod_rename_rule", { from, to });
export const modDeleteRule = (path: string) => invoke<void>("mod_delete_rule", { path });
export const modListTables = () => invoke<TableEntry[]>("mod_list_tables");
export const modReadTable = (name: string) => invoke<string>("mod_read_table", { name });
export const modCheckTable = (text: string) => invoke<Diagnostic[]>("mod_check_table", { text });
export const modSaveTable = (name: string, text: string) => invoke<Diagnostic[]>("mod_save_table", { name, text });
export const modNewTable = (name: string) => invoke<TableEntry>("mod_new_table", { name });
/// The renamed table and how many reads in rule files were changed with it.
export const modRenameTable = (from: string, to: string) =>
  invoke<[TableEntry, number]>("mod_rename_table", { from, to });
export const modDeleteTable = (name: string) => invoke<void>("mod_delete_table", { name });
export const modGetPaths = () => invoke<ModPaths>("mod_get_paths");
export const modSetPaths = (modsFolder: string | null, declaredSet: string | null) =>
  invoke<ModPaths>("mod_set_paths", { modsFolder, declaredSet });
export const modConflicts = () => invoke<ConflictReport>("mod_conflicts");

// Code mods in C (core/src/codemod.rs).
export interface SourceFile {
  path: string;
  bytes: number;
}

export interface BuildDiagnostic {
  /// Relative to the mod; empty for a message about the whole build.
  file: string;
  /// 1-based; 0 when unknown.
  line: number;
  column: number;
  severity: "error" | "warning" | "note";
  message: string;
}

export interface BuildResult {
  ok: boolean;
  wasmPath: string | null;
  size: number;
  millis: number;
  diagnostics: BuildDiagnostic[];
  log: string;
}

export interface ZigStatus {
  path: string | null;
  source: "own" | "downloaded" | "path" | null;
  version: string | null;
  pinned: string;
  downloadMb: number | null;
  chosen: string | null;
}

export interface ZigProgress {
  phase: "download" | "check" | "unpack";
  done: number;
  total: number;
}

export interface CodeApi {
  functions: { name: string; signature: string; doc: string }[];
  constants: { name: string; value: string; group: string }[];
}

export const zigStatus = () => invoke<ZigStatus>("zig_status");
export const setZigPath = (path: string | null) => invoke<ZigStatus>("set_zig_path", { path });
export const fetchZig = () => invoke<ZigStatus>("fetch_zig");
export const newCodeMod = () => invoke<string[]>("new_code_mod");
export const buildCodeMod = () => invoke<BuildResult>("build_code_mod");
export const modCodeFiles = () => invoke<SourceFile[]>("mod_code_files");
export const modCodeRead = (path: string) => invoke<string>("mod_code_read", { path });
export const modCodeWrite = (path: string, text: string) => invoke<void>("mod_code_write", { path, text });
export const modCodeNew = (path: string) => invoke<void>("mod_code_new", { path });
export const modCodeRename = (from: string, to: string) => invoke<void>("mod_code_rename", { from, to });
export const modCodeDelete = (path: string) => invoke<void>("mod_code_delete", { path });
export const modCodeApi = () => invoke<CodeApi>("mod_code_api");
