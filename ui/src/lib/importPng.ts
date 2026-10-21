// The import flow: pick a PNG file, then confirm in the dialog.
import { open } from "@tauri-apps/plugin-dialog";
import { mount, unmount } from "svelte";
import ImportPicture, { type ImportTarget } from "./ImportPicture.svelte";

/// Asks for a PNG file, then opens the confirm step. `onDone` runs after
/// the picture is written.
export async function importPng(target: ImportTarget, onDone: () => void) {
  const path = await open({ title: "Choose a PNG", filters: [{ name: "PNG picture", extensions: ["png"] }] });
  if (typeof path !== "string") return;
  const host = mount(ImportPicture, {
    target: document.body,
    props: { target, path, onDone, onClose: () => unmount(host) },
  });
}
