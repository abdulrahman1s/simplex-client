// Open-tunnel leases for one client: the counting rule behind the per-IP Durable Object.
// A lease is removed when its tunnel closes; its expiry only cleans up after a lost release.

export interface LeaseStore {
  entries(): Iterable<[string, number]>;
  put(id: string, expiresAt: number): void;
  delete(id: string): void;
}

/** Drops expired leases and returns the earliest remaining expiry, if any. */
export function prune(store: LeaseStore, now: number): number | null {
  let next: number | null = null;
  for (const [id, expiresAt] of [...store.entries()]) {
    if (expiresAt <= now) store.delete(id);
    else next = next === null ? expiresAt : Math.min(next, expiresAt);
  }
  return next;
}

export function count(store: LeaseStore): number {
  return [...store.entries()].length;
}

/** Takes a lease unless `max` are already held. */
export function acquire(store: LeaseStore, id: string, max: number, expiresAt: number, now: number): boolean {
  prune(store, now);
  if (count(store) >= max) return false;
  store.put(id, expiresAt);
  return true;
}
