// One Save and Discard per entry. A screen that edits an entry provides
// EntryParts; panels inside it (Name and text, In battle) register what they
// have changed and how to save or drop it, instead of showing their own
// buttons. A panel used where no screen provides it keeps its own buttons.
import { getContext, setContext } from "svelte";
import { SvelteMap } from "svelte/reactivity";

export interface Part {
  /// Whether the panel has unsaved edits (read reactively).
  dirty: () => boolean;
  save: () => Promise<void>;
  discard: () => void;
}

export class EntryParts {
  #parts = new SvelteMap<string, Part>();

  /// Adds a panel; returns the function that removes it.
  register(key: string, part: Part): () => void {
    this.#parts.set(key, part);
    return () => {
      if (this.#parts.get(key) === part) this.#parts.delete(key);
    };
  }

  get dirty(): boolean {
    for (const p of this.#parts.values()) if (p.dirty()) return true;
    return false;
  }

  async save(): Promise<void> {
    for (const p of this.#parts.values()) if (p.dirty()) await p.save();
  }

  discard(): void {
    for (const p of this.#parts.values()) p.discard();
  }
}

const KEY = Symbol("entry-parts");

export function provideParts(): EntryParts {
  return setContext(KEY, new EntryParts());
}

export function useParts(): EntryParts | undefined {
  return getContext<EntryParts | undefined>(KEY);
}
