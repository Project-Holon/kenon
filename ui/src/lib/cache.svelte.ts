// Front-end caches and helpers, so a view does not ask the back end again
// for what it already has. Listings that change with an edit (changed
// flags) are dropped by `forget` after a save.

const store = new Map<string, Promise<unknown>>();

/// The result of `load` for `key`, asked once.
export function once<T>(key: string, load: () => Promise<T>): Promise<T> {
  let p = store.get(key) as Promise<T> | undefined;
  if (!p) {
    p = load();
    store.set(key, p);
    // A failure is not kept: the next call tries again.
    p.catch(() => store.delete(key));
  }
  return p;
}

/// Drops cached results whose key starts with one of the prefixes.
export function forget(...prefixes: string[]) {
  for (const key of [...store.keys()]) if (prefixes.some((p) => key.startsWith(p))) store.delete(key);
}

/// Drops everything (a new game or project).
export function forgetAll() {
  store.clear();
}

/// A value that follows `get` after `ms` of quiet, for search boxes.
export function debounced<T>(get: () => T, ms = 150) {
  let value = $state(get());
  $effect(() => {
    const next = get();
    const timer = setTimeout(() => (value = next), ms);
    return () => clearTimeout(timer);
  });
  return {
    get value() {
      return value;
    },
  };
}
