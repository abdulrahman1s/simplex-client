import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { DEFAULT_LIMITS } from "../src/limits";

/** `SEND_AHEAD` in the Rust file client, evaluated from its `a * b` form. */
function rustSendAhead(): number {
  const path = new URL("../../src/engine/files/runner/link.rs", import.meta.url);
  const m = readFileSync(path, "utf8").match(/const SEND_AHEAD: u32 = (\d+) \* (\d+);/);
  if (!m) throw new Error("SEND_AHEAD not found in link.rs");
  return Number(m[1]) * Number(m[2]);
}

describe("the Rust client's upload window", () => {
  it("leaves a quarter of the tunnel's write-ahead allowance for TLS, HTTP/2 framing and control frames", () => {
    // Unacknowledged upload bytes are the most a client can have queued in the Worker.
    expect(rustSendAhead()).toBeLessThanOrEqual(DEFAULT_LIMITS.maxQueuedBytes * 0.75);
  });
});
