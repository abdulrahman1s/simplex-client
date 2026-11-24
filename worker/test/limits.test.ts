import { describe, expect, it } from "vitest";
import { ipKey } from "../src/ipkey";
import { acquire, count, prune, type LeaseStore } from "../src/leases";
import { DEFAULT_LIMITS, Meter, readLimits } from "../src/limits";

describe("readLimits", () => {
  it("defaults to 8 tunnels, 10 s hello, 15 min idle, 24 h lifetime, 1 GiB and 64 KiB frames", () => {
    expect(readLimits({})).toEqual({
      maxTunnelsPerIp: 8,
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
      HELLO_TIMEOUT_SECONDS: "5",
      IDLE_TIMEOUT_SECONDS: "60",
      MAX_LIFETIME_SECONDS: "3600",
      MAX_BYTES_PER_TUNNEL: "1000",
      MAX_FRAME_BYTES: "16384",
    });
    expect(l).toMatchObject({ maxTunnelsPerIp: 3, helloTimeoutMs: 5000, idleTimeoutMs: 60_000, maxLifetimeMs: 3_600_000, maxBytes: 1000, maxFrameBytes: 16384 });
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

function memStore(): LeaseStore & { map: Map<string, number> } {
  const map = new Map<string, number>();
  return { map, entries: () => map.entries(), put: (id, t) => void map.set(id, t), delete: (id) => void map.delete(id) };
}

describe("leases", () => {
  it("caps open tunnels per client and frees a slot on release", () => {
    const s = memStore();
    for (let i = 0; i < 8; i++) expect(acquire(s, `t${i}`, 8, 1000, 0)).toBe(true);
    expect(acquire(s, "t8", 8, 1000, 0)).toBe(false);
    s.delete("t3");
    expect(acquire(s, "t8", 8, 1000, 0)).toBe(true);
    expect(count(s)).toBe(8);
  });

  it("drops leases whose release was lost once they expire", () => {
    const s = memStore();
    acquire(s, "a", 2, 100, 0);
    acquire(s, "b", 2, 200, 0);
    expect(acquire(s, "c", 2, 300, 50)).toBe(false);
    expect(acquire(s, "c", 2, 300, 100)).toBe(true);
    expect(prune(s, 150)).toBe(200);
    expect(prune(s, 400)).toBeNull();
    expect(count(s)).toBe(0);
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
