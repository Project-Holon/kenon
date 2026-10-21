// The data table commands (core/src/tables.rs) and an enum name cache.
import { invoke } from "@tauri-apps/api/core";
import { once } from "./cache.svelte";

export interface Value {
  key: string;
  label: string;
  value: number;
  min: number;
  max: number;
  enum: string | null;
  readonly: boolean;
  inferred: boolean;
  note: string | null;
}

export interface Column {
  name: string;
  label: string;
  min: number;
  max: number;
  enum: string | null;
}

export type Section =
  | { kind: "fields"; label: string; values: Value[] }
  | { kind: "list"; key: string; label: string; columns: Column[]; rows: number[][]; fixed: boolean; maxRows: number };

export interface TableRecord {
  sections: Section[];
  seasons: number;
  changed: boolean;
}

export interface Entry {
  index: number;
  name: string;
}

export interface Edit {
  values: Record<string, number>;
  lists: Record<string, number[][]>;
  season: number;
}

export const tableEntries = (table: string) =>
  once(`entries/${table}`, () => invoke<Entry[]>("table_entries", { table }));
export const tableRecord = (table: string, index: number, season = 0) =>
  invoke<TableRecord>("table_record", { table, index, season });
export const saveRecord = (table: string, index: number, edit: Edit) =>
  invoke<void>("save_record", { table, index, edit });

const enums = new Map<string, Promise<string[]>>();

/// The names of an enum, read from the ROM once per game.
export function enumValues(name: string): Promise<string[]> {
  let p = enums.get(name);
  if (!p) {
    p = invoke<string[]>("enum_values", { name }).catch(() => []);
    enums.set(name, p);
  }
  return p;
}

export function clearEnums() {
  enums.clear();
}
