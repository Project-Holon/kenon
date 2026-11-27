// The language kenon edits text in (eidos 0.2.8, `<bank>.<lang>.msg`): the
// ROM's own (base) or a translation. Set from the top bar.
import { invoke } from "@tauri-apps/api/core";

export const LANGUAGES: [string, string][] = [
  ["en", "English"],
  ["fr", "French"],
  ["de", "German"],
  ["it", "Italian"],
  ["es", "Spanish"],
];

export const textLang = $state<{ base: string; current: string; references: Record<string, string> }>({
  base: "en",
  current: "en",
  references: {},
});

/// Whether text is being edited in a translation, not the ROM's language.
export const translating = () => textLang.current !== textLang.base;

export async function loadLanguages() {
  const l = await invoke<{ base: string | null; references: Record<string, string> }>("languages");
  const base = l.base ?? "en";
  if (textLang.current === textLang.base) textLang.current = base;
  textLang.base = base;
  textLang.references = l.references;
}

export interface Reference {
  source: "rom" | "bundled" | "bundledLater";
  lines: Record<string, string>;
}

const cache = new Map<string, Promise<Reference | null>>();
/// The game's own text of a bank in a language (a reference ROM's, else
/// kenon's bundled names and descriptions).
export function referenceText(lang: string, arc: number, bank: number): Promise<Reference | null> {
  const key = `${lang}/${arc}/${bank}`;
  if (!cache.has(key)) cache.set(key, invoke<Reference | null>("reference_text", { lang, arc, bank }).catch(() => null));
  return cache.get(key)!;
}
export const clearReferences = () => cache.clear();
