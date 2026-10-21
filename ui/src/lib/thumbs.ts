// Thumbnails load in the order they are asked for, a few at a time, so the
// first rows of a list show at once and the rest follow in order instead of
// every picture of an archive being decoded together. Loaded pictures are
// kept for the session.

const PARALLEL = 4;
const done = new Map<string, Promise<string>>();
const waiting: { url: string; resolve: (u: string) => void; reject: () => void; cancelled: boolean }[] = [];
let running = 0;

function pump() {
  while (running < PARALLEL && waiting.length) {
    const job = waiting.shift()!;
    if (job.cancelled) continue;
    running++;
    fetch(job.url)
      .then((r) => (r.ok ? r.blob() : Promise.reject()))
      .then((b) => job.resolve(URL.createObjectURL(b)))
      .catch(() => job.reject())
      .finally(() => {
        running--;
        pump();
      });
  }
}

/// A loaded picture for `url`; `signal` drops it from the queue when the
/// row scrolls away before its turn.
export function thumb(url: string, signal?: AbortSignal): Promise<string> {
  const known = done.get(url);
  if (known) return known;
  const p = new Promise<string>((resolve, reject) => {
    const job = { url, resolve, reject, cancelled: false };
    signal?.addEventListener("abort", () => {
      if (!job.cancelled) {
        job.cancelled = true;
        done.delete(url);
        reject();
      }
    });
    waiting.push(job);
    pump();
  });
  done.set(url, p);
  p.catch(() => done.delete(url));
  return p;
}

/// Forgets loaded pictures (after an edit changed them).
export function forgetThumbs() {
  for (const p of done.values()) p.then((u) => URL.revokeObjectURL(u)).catch(() => {});
  done.clear();
}
