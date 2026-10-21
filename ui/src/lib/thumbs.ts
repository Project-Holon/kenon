// Thumbnails load in the order they are asked for, a few at a time, so the
// first rows of a list show at once and the rest follow in order instead of
// every picture of an archive being decoded together. Loaded pictures are
// kept for the session.

const PARALLEL = 4;

interface Job {
  url: string;
  promise: Promise<string>;
  resolve: (u: string) => void;
  reject: () => void;
  /// How many callers still want it.
  refs: number;
  started: boolean;
  cancelled: boolean;
}

const done = new Map<string, Job>();
const waiting: Job[] = [];
let running = 0;

function pump() {
  while (running < PARALLEL && waiting.length) {
    const job = waiting.shift()!;
    if (job.cancelled) continue;
    job.started = true;
    running++;
    fetch(job.url)
      .then((r) => (r.ok ? r.blob() : Promise.reject()))
      .then((b) => job.resolve(URL.createObjectURL(b)))
      .catch(() => {
        // A failed picture may be asked for again.
        if (done.get(job.url) === job) done.delete(job.url);
        job.reject();
      })
      .finally(() => {
        running--;
        pump();
      });
  }
}

/// A loaded picture for `url`; `signal` drops it from the queue when the
/// row scrolls away before its turn. A picture already on its way or loaded
/// stays cached for the next caller.
export function thumb(url: string, signal?: AbortSignal): Promise<string> {
  let job = done.get(url);
  if (!job) {
    let resolve!: (u: string) => void;
    let reject!: () => void;
    const promise = new Promise<string>((res, rej) => {
      resolve = res;
      reject = rej;
    });
    promise.catch(() => {});
    job = { url, promise, resolve, reject, refs: 0, started: false, cancelled: false };
    done.set(url, job);
    waiting.push(job);
  }
  const mine = job;
  mine.refs++;
  signal?.addEventListener("abort", () => {
    mine.refs--;
    // Only a picture nobody wants and not yet fetched is dropped.
    if (mine.refs <= 0 && !mine.started && !mine.cancelled) {
      mine.cancelled = true;
      if (done.get(url) === mine) done.delete(url);
      mine.reject();
    }
  });
  pump();
  return mine.promise;
}

/// Forgets loaded pictures (after an edit changed them).
export function forgetThumbs() {
  for (const j of done.values()) j.promise.then((u) => URL.revokeObjectURL(u)).catch(() => {});
  done.clear();
}
