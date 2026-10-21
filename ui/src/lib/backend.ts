// Typed wrappers for the commands in app/src/lib.rs.
import { invoke } from "@tauri-apps/api/core";

export interface DataRoot {
  chosen: string;
  dataDir: string;
  extractionDir: string | null;
  hasManifest: boolean;
  hasArm9: boolean;
}

export type DataRootCheck =
  | { state: "missing"; path: string }
  | { state: "notAFolder"; path: string }
  | { state: "noGameData"; path: string }
  | ({ state: "ready" } & DataRoot);

export interface DataFolderStatus {
  folder: string | null;
  check: DataRootCheck | null;
  suggestion: string | null;
}

export function dataFolderStatus(): Promise<DataFolderStatus> {
  return invoke("data_folder_status");
}

export function setDataFolder(folder: string | null): Promise<DataFolderStatus> {
  return invoke("set_data_folder", { folder });
}
