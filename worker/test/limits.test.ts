import { describe, expect, it } from "vitest";
import { ipKey } from "../src/ipkey";
import { admit, emptyState, nextExpiry, prune, release, renew, START_WINDOW_MS } from "../src/leases";
import { DEFAULT_LIMITS, Meter, readLimits } from "../src/limits";

describe("readLimits", () => {
  it("defaults to 8 tunnels, 10 s hello, 15 min idle, 24 h lifetime, 1 GiB and 64 KiB frames", () => {
    expect(readLimits({})).toEqual({
      maxTunnelsPerIp: 8,
      maxNewTunnelsPerMinute: 30,
      helloTimeoutMs: 10_000,
      idleTimeoutMs: 900_000,
      maxLifetimeMs: 86_400_000,
      maxBytes: 1_073_741_824,
      maxFrameBytes: 65_536,
      maxQueuedBytes: DEFAULT_LIMITS.maxQueuedBytes,
    });
  });

  it("reads the vars as seconds and bytes", () => {
    const l = readLimits({
      MAX_TUNNELS_PER_IP: "3",
      MAX_NEW_TUNNELS_PER_MINUTE: "12",
      HELLO_TIMEOUT_SECONDS: "5",
      IDLE_TIMEOUT_SECONDS: "60",
      MAX_LIFETIME_SECONDS: "3600",
      MAX_BYTES_PER_TUNNEL: "1000",
      MAX_FRAME_BYTES: "16384",
    });
    expect(l).toMatchObject({ maxTunnelsPerIp: 3, maxNewTunnelsPerMinute: 12, helloTimeoutMs: 5000, idleTimeoutMs: 60_000, maxLifetimeMs: 3_600_000, maxBytes: 1000, maxFrameBytes: 16384 });
  });

  it("falls back to the default for empty, malformed or non-positive values", () => {
    const l = readLimits({ MAX_TUNNELS_PER_IP: "", IDLE_TIMEOUT_SECONDS: "soon", MAX_BYTES_PER_TUNNEL: "-1", MAX_FRAME_BYTES: "0" });
    expect(l).toEqual(DEFAULT_LIMITS);
  });
});

describe("Meter", () => {
  const limits = { ...DEFAULT_LIMITS, idleTimeoutMs: 1000, maxLifetimeMs: 10_000, maxBytes: 100 };

  it("closes with quota once the byte budget, counted both ways, is spent", () => {
    const m = new Meter(limits, 0);
    expect(m.add(60, 1)).toBe(true);
    expect(m.add(40, 2)).toBe(true);
    expect(m.add(1, 3)).toBe(false);
  });

  it("expires after the idle timeout with no bytes either way", () => {
    const m = new Meter(limits, 0);
    expect(m.expired(999)).toBeNull();
    expect(m.expired(1000)).toBe("idle");
    m.add(1, 900);
    expect(m.expired(1000)).toBeNull();
    expect(m.nextCheck(1000)).toBe(900);
  });

  it("expires at the maximum lifetime even while busy", () => {
    const m = new Meter(limits, 0);
    for (let t = 500; t < 10_000; t += 500) m.add(0, t);
    expect(m.expired(9_999)).toBeNull();
    expect(m.expired(10_000)).toBe("lifetime");
    expect(m.nextCheck(9_800)).toBe(200);
  });
});

describe("counter admission", () => {
  const rules = { maxOpen: 8, maxStarts: 30, leaseMs: 1000 };

  it("caps open tunnels per client and frees a slot on release", () => {
    const s = emptyState();
    for (let i = 0; i < 8; i++) expect(admit(s, `t${i}`, rules, 0)).toBe("ok");
    expect(admit(s, "t8", rules, 0)).toBe("too_many_tunnels");
    release(s, "t3");
    expect(admit(s, "t8", rules, 0)).toBe("ok");
    expect(Object.keys(s.leases)).toHaveLength(8);
  });

  it("limits new tunnels per minute, counting refused ones, and forgets starts after the window", () => {
    const s = emptyState();
    const tight = { ...rules, maxOpen: 1, maxStarts: 3 };
    expect(admit(s, "a", tight, 0)).toBe("ok");
    expect(admit(s, "b", tight, 1)).toBe("too_many_tunnels");
    release(s, "a");
    expect(admit(s, "c", tight, 2)).toBe("ok");
    release(s, "c");
    expect(admit(s, "d", tight, 3)).toBe("rate_limited");
    expect(admit(s, "d", tight, START_WINDOW_MS)).toBe("ok");
  });

  it("lets an unrenewed lease lapse and keeps a renewed one", () => {
    const s = emptyState();
    admit(s, "live", rules, 0);
    admit(s, "lost", rules, 0);
    renew(s, "live", 900, 1000);
    prune(s, 1500);
    expect(Object.keys(s.leases)).toEqual(["live"]);
  });

  it("reports when it next changes, and nothing once empty", () => {
    const s = emptyState();
    expect(nextExpiry(s)).toBeNull();
    admit(s, "a", rules, 0);
    expect(nextExpiry(s)).toBe(1000);
    release(s, "a");
    expect(nextExpiry(s)).toBe(START_WINDOW_MS);
    prune(s, START_WINDOW_MS);
    expect(nextExpiry(s)).toBeNull();
  });
});

describe("ipKey", () => {
  it("is a stable hash that never contains the address", async () => {
    const a = await ipKey("203.0.113.7", "salt");
    expect(a).toMatch(/^[0-9a-f]{32}$/);
    expect(a).toBe(await ipKey("203.0.113.7", "salt"));
    expect(a).not.toContain("203");
  });

  it("changes with the salt and with the address", async () => {
    const a = await ipKey("203.0.113.7", "salt");
    expect(await ipKey("203.0.113.7", "other")).not.toBe(a);
    expect(await ipKey("203.0.113.8", "salt")).not.toBe(a);
    expect(await ipKey("203.0.113.7", undefined)).not.toBe(a);
  });
});
