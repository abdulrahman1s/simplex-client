import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { allowedHosts, PRESET_HOSTS } from "../src/hosts";
import { checkTarget, parsePorts } from "../src/validate";

/** The clearnet host of every `smp://` or `xftp://` address in a Rust source file. */
function rustHosts(path: string): string[] {
  const text = readFileSync(new URL(path, import.meta.url), "utf8");
  return [...text.matchAll(/"(?:smp|xftp):\/\/[^@"]+@([^,":/"]+)/g)].map((m) => m[1]);
}

const policy = (allowed?: string) => ({ ports: parsePorts(undefined), hosts: allowedHosts(allowed) });

describe("default hosts", () => {
  it("are exactly the preset SMP relays and XFTP servers of the Rust client", () => {
    const rust = [...rustHosts("../../src/engine/presets.rs"), ...rustHosts("../../src/xftp/servers.rs")];
    expect(rust.length).toBe(32);
    expect([...PRESET_HOSTS].sort()).toEqual([...new Set(rust)].sort());
  });

  it("let a preset relay through and refuse any other host without ALLOWED_HOSTS", () => {
    expect(checkTarget("smp4.simplex.im", 5223, policy(""))).toEqual({ host: "smp4.simplex.im" });
    expect(checkTarget("xftp3.simplexonflux.com", 443, policy(undefined))).toEqual({ host: "xftp3.simplexonflux.com" });
    expect(checkTarget("my-vps.example", 443, policy(""))).toEqual({ error: "forbidden_host" });
    expect(checkTarget("93.184.215.14", 443, policy(""))).toEqual({ error: "forbidden_host" });
  });

  it("grow with ALLOWED_HOSTS, and `*` opens any public host but no private one", () => {
    expect(checkTarget("relay.example.org", 5223, policy("relay.example.org"))).toEqual({ host: "relay.example.org" });
    expect(checkTarget("93.184.215.14", 443, policy("*"))).toEqual({ host: "93.184.215.14" });
    expect(checkTarget("10.0.0.1", 443, policy("*"))).toEqual({ error: "forbidden_host" });
  });
});
