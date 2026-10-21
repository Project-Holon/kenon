// Saving to disk and copying: the destination dialogs and the commands
// that write the files (app/src/lib.rs).
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { toast } from "./menu.svelte";

/// A file name from a label: no characters Windows refuses.
export function fileName(label: string): string {
  return label.replace(/[\/:*?"<>|\u0000-\u001f]/g, " ").replace(/\s+/g, " ").trim() || "file";
}

const baseName = (path: string) => path.split(/[\/]/).pop() ?? path;

export async function copyText(text: string, what = "Copied") {
  try {
    await navigator.clipboard.writeText(text);
    toast(what);
  } catch {
    toast("Could not copy.", true);
  }
}

async function ask(name: string, ext: string, kind: string): Promise<string | null> {
  return save({ defaultPath: name, filters: [{ name: kind, extensions: [ext] }] });
}

/// Saves a bridge asset (see core/src/bridge.rs) after asking where.
export async function saveAsset(path: string, name: string, ext: string, kind: string) {
  const dest = await ask(`${fileName(name)}.${ext}`, ext, kind);
  if (!dest) return;
  await invoke("export_asset", { path, dest });
  toast(`Saved ${baseName(dest)}`);
}

export const savePng = (path: string, name: string) => saveAsset(path, name, "png", "PNG picture");
export const saveWav = (path: string, name: string) => saveAsset(path, name, "wav", "WAV sound");

/// Saves several assets into a folder the user picks: `files` are
/// [bridge path, file name] pairs.
export async function saveAssets(files: [string, string][]) {
  const dir = await open({ directory: true, title: "Choose a folder" });
  if (typeof dir !== "string") return;
  const sep = dir.includes("\\") ? "\\" : "/";
  for (const [path, name] of files) {
    await invoke("export_asset", { path, dest: `${dir}${sep}${fileName(name)}` });
  }
  toast(`Saved ${files.length} file${files.length === 1 ? "" : "s"}`);
}

/// Saves a game file: decompressed, or as stored when `raw`.
export async function saveMember(arc: number, index: number, name: string, ext = "bin", raw = false) {
  const dest = await ask(`${fileName(name)}.${ext}`, ext, "Game file");
  if (!dest) return;
  await invoke("export_member", { arc, index, dest, raw });
  toast(`Saved ${baseName(dest)}`);
}

export async function saveText(text: string, name: string, ext = "txt", kind = "Text") {
  const dest = await ask(`${fileName(name)}.${ext}`, ext, kind);
  if (!dest) return;
  await invoke("export_text", { dest, text });
  toast(`Saved ${baseName(dest)}`);
}

/// Makes a menu action out of an export: a failure shows as a note.
export function guarded<A extends unknown[]>(fn: (...args: A) => Promise<void>) {
  return async (...args: A) => {
    try {
      await fn(...args);
    } catch (e) {
      toast(String(e), true);
    }
  };
}

/// The file extension a game file kind is usually saved with.
export function memberExt(kind: string): string {
  return /^[A-Z0-9]{3,4}$/.test(kind) ? kind.toLowerCase() : "bin";
}

/// Asks the workspace to show a game file in the Files list.
export function openInFiles(arc: number, index?: number) {
  filesTarget.arc = arc;
  filesTarget.index = index ?? null;
  window.dispatchEvent(new CustomEvent("kenon:open", { detail: { category: "files" } }));
}

/// The archive (and member) the Files list opens on; read once when it mounts.
export const filesTarget: { arc: number | null; index: number | null } = { arc: null, index: null };
