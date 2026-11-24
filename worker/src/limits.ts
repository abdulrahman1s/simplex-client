// Per-tunnel limits read from the deployment's vars, and the meter that enforces them.

export interface Limits {
  maxTunnelsPerIp: number;
  maxNewTunnelsPerMinute: number;
  helloTimeoutMs: number;
  idleTimeoutMs: number;
  maxLifetimeMs: number;
  maxBytes: number;
  maxFrameBytes: number;
  /** Client bytes waiting for the relay socket; past this the client is not honouring flow. */
  maxQueuedBytes: number;
}

export interface LimitVars {
  MAX_TUNNELS_PER_IP?: string;
  MAX_NEW_TUNNELS_PER_MINUTE?: string;
  HELLO_TIMEOUT_SECONDS?: string;
  IDLE_TIMEOUT_SECONDS?: string;
  MAX_LIFETIME_SECONDS?: string;
  MAX_BYTES_PER_TUNNEL?: string;
  MAX_FRAME_BYTES?: string;
}

export const DEFAULT_LIMITS: Limits = {
  maxTunnelsPerIp: 8,
  maxNewTunnelsPerMinute: 30,
  helloTimeoutMs: 10_000,
  idleTimeoutMs: 15 * 60_000,
  maxLifetimeMs: 24 * 3_600_000,
  maxBytes: 1024 ** 3,
  maxFrameBytes: 64 * 1024,
  maxQueuedBytes: 1024 * 1024,
};

/** A missing, malformed or non-positive value falls back to the default. */
export function readLimits(vars: LimitVars): Limits {
  const num = (v: string | undefined, fallback: number, scale = 1) => {
    const n = Number(v);
    return v !== undefined && v.trim() !== "" && Number.isFinite(n) && n > 0 ? n * scale : fallback;
  };
  const d = DEFAULT_LIMITS;
  return {
    maxTunnelsPerIp: Math.floor(num(vars.MAX_TUNNELS_PER_IP, d.maxTunnelsPerIp)),
    maxNewTunnelsPerMinute: Math.floor(num(vars.MAX_NEW_TUNNELS_PER_MINUTE, d.maxNewTunnelsPerMinute)),
    helloTimeoutMs: num(vars.HELLO_TIMEOUT_SECONDS, d.helloTimeoutMs, 1000),
    idleTimeoutMs: num(vars.IDLE_TIMEOUT_SECONDS, d.idleTimeoutMs, 1000),
    maxLifetimeMs: num(vars.MAX_LIFETIME_SECONDS, d.maxLifetimeMs, 1000),
    maxBytes: num(vars.MAX_BYTES_PER_TUNNEL, d.maxBytes),
    maxFrameBytes: num(vars.MAX_FRAME_BYTES, d.maxFrameBytes),
    maxQueuedBytes: d.maxQueuedBytes,
  };
}

export type Expiry = "idle" | "lifetime";

/** Bytes, idle time and age of one tunnel. Times are milliseconds from the caller's clock. */
export class Meter {
  private used = 0;
  private last: number;

  constructor(
    private readonly limits: Limits,
    private readonly opened: number,
  ) {
    this.last = opened;
  }

  /** Counts `n` bytes in either direction; false once the tunnel's budget is spent. */
  add(n: number, now: number): boolean {
    this.used += n;
    this.last = now;
    return this.used <= this.limits.maxBytes;
  }

  expired(now: number): Expiry | null {
    if (now - this.opened >= this.limits.maxLifetimeMs) return "lifetime";
    if (now - this.last >= this.limits.idleTimeoutMs) return "idle";
    return null;
  }

  /** Milliseconds until the next limit can trip. */
  nextCheck(now: number): number {
    const lifetime = this.opened + this.limits.maxLifetimeMs;
    const idle = this.last + this.limits.idleTimeoutMs;
    return Math.max(0, Math.min(lifetime, idle) - now);
  }
}
