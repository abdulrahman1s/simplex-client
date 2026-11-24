// The admission rule behind the per-IP Durable Object: open tunnels hold leases, and recent
// starts are counted in a sliding window. Open tunnels renew their lease, so a lost release
// frees its slot within one lease period.

export interface CounterState {
  /** Tunnel id to lease expiry. */
  leases: Record<string, number>;
  /** When recent tunnels were started, oldest first. */
  starts: number[];
}

export interface CounterRules {
  maxOpen: number;
  /** Starts allowed per START_WINDOW_MS. */
  maxStarts: number;
  leaseMs: number;
}

export const START_WINDOW_MS = 60_000;

export type Admission = "ok" | "too_many_tunnels" | "rate_limited";

export function emptyState(): CounterState {
  return { leases: {}, starts: [] };
}

/** Drops expired leases and starts older than the window. */
export function prune(s: CounterState, now: number): void {
  for (const [id, expiresAt] of Object.entries(s.leases)) if (expiresAt <= now) delete s.leases[id];
  s.starts = s.starts.filter((t) => t > now - START_WINDOW_MS);
}

/** Counts a start, then takes a lease unless the client is over either limit. */
export function admit(s: CounterState, id: string, rules: CounterRules, now: number): Admission {
  prune(s, now);
  if (s.starts.length >= rules.maxStarts) return "rate_limited";
  s.starts.push(now);
  if (Object.keys(s.leases).length >= rules.maxOpen) return "too_many_tunnels";
  s.leases[id] = now + rules.leaseMs;
  return "ok";
}

/** Extends a live tunnel's lease, or takes it again if it lapsed; renewal never hits a limit. */
export function renew(s: CounterState, id: string, now: number, leaseMs: number): void {
  s.leases[id] = now + leaseMs;
}

export function release(s: CounterState, id: string): void {
  delete s.leases[id];
}

/** When the state next changes on its own, or null when it holds nothing. */
export function nextExpiry(s: CounterState): number | null {
  const times = [...Object.values(s.leases), ...s.starts.map((t) => t + START_WINDOW_MS)];
  return times.length === 0 ? null : Math.min(...times);
}
