import { describe, expect, it } from "vitest";
import {
  checkTarget,
  originAllowed,
  parseHello,
  parseIpv6,
  parseList,
  parsePorts,
  tokenEquals,
  type Policy,
} from "../src/validate";

const open: Policy = { ports: parsePorts(undefined), hosts: [] };
const target = (host: string, port = 5223, policy = open) => checkTarget(host, port, policy);

describe("parseHello", () => {
  it("reads host, port and an optional token", () => {
    expect(parseHello('{"host":"smp4.simplex.im","port":5223}')).toEqual({ host: "smp4.simplex.im", port: 5223 });
    expect(parseHello('{"host":"a.b","port":443,"token":"t"}')).toEqual({ host: "a.b", port: 443, token: "t" });
  });

  it("rejects malformed or oversized frames", () => {
    for (const bad of [
      "",
      "nope",
      "[]",
      "null",
      '{"host":1,"port":5223}',
      '{"host":"a.b","port":"5223"}',
      '{"host":"a.b","port":0}',
      '{"host":"a.b","port":65536}',
      '{"host":"a.b","port":52.5}',
      '{"host":"a.b","port":5223,"token":5}',
      JSON.stringify({ host: "a".repeat(2000), port: 5223 }),
    ]) {
      expect(parseHello(bad), bad.slice(0, 40)).toBeNull();
    }
  });
});

describe("checkTarget ports", () => {
  it("allows 5223 and 443 by default and nothing else", () => {
    expect(target("smp4.simplex.im", 5223)).toEqual({ host: "smp4.simplex.im" });
    expect(target("smp4.simplex.im", 443)).toEqual({ host: "smp4.simplex.im" });
    for (const port of [22, 25, 80, 6379, 8443]) {
      expect(target("smp4.simplex.im", port)).toEqual({ error: "forbidden_port" });
    }
  });

  it("follows ALLOWED_PORTS and ignores junk entries", () => {
    expect([...parsePorts("5223, 7000,abc,0,70000")]).toEqual([5223, 7000]);
  });
});

describe("checkTarget hosts", () => {
  it("normalizes case, a trailing dot and IPv6 brackets", () => {
    expect(target("SMP4.SimpleX.im.")).toEqual({ host: "smp4.simplex.im" });
    expect(target("[2606:4700::1]")).toEqual({ host: "2606:4700::1" });
  });

  it("allows public IPv4 and IPv6 literals", () => {
    expect(target("1.1.1.1")).toEqual({ host: "1.1.1.1" });
    expect(target("2a01:4f8::1")).toEqual({ host: "2a01:4f8::1" });
  });

  it("rejects private, loopback, link-local and CGNAT IPv4", () => {
    for (const ip of ["10.0.0.1", "127.0.0.1", "169.254.169.254", "172.16.0.1", "172.31.255.255", "192.168.1.1", "100.64.0.1", "100.127.255.255", "0.0.0.0", "224.0.0.1", "255.255.255.255"]) {
      expect(target(ip), ip).toEqual({ error: "forbidden_host" });
    }
    expect(target("172.32.0.1")).toEqual({ host: "172.32.0.1" });
    expect(target("100.128.0.1")).toEqual({ host: "100.128.0.1" });
  });

  it("rejects non-canonical numeric addresses", () => {
    for (const ip of ["127.1", "2130706433", "0x7f.0.0.1", "0x7f000001", "010.0.0.1", "1.2.3.4.5", "1.2.3.256"]) {
      expect("error" in target(ip), ip).toBe(true);
    }
  });

  it("rejects private, loopback, link-local and embedded-private IPv6", () => {
    for (const ip of ["::", "::1", "[::1]", "fe80::1", "fc00::1", "fd12:3456::1", "fec0::1", "ff02::1", "::ffff:127.0.0.1", "::ffff:7f00:1", "::ffff:10.0.0.1", "64:ff9b::a00:1", "64:ff9b:1::1", "2002:a00:1::1", "2001:db8::1", "::127.0.0.1"]) {
      expect(target(ip), ip).toEqual({ error: "forbidden_host" });
    }
    expect(target("::ffff:1.1.1.1")).toEqual({ host: "::ffff:1.1.1.1" });
  });

  it("rejects malformed IPv6 and zone ids", () => {
    for (const ip of ["fe80::1%eth0", "1::2::3", "12345::1", "1:2:3:4:5:6:7:8:9", ":::"]) {
      expect(target(ip), ip).toEqual({ error: "bad_request" });
    }
  });

  it("rejects local names", () => {
    for (const host of ["localhost", "foo.localhost", "printer.local", "db.internal", "metadata", "intranet"]) {
      expect(target(host), host).toEqual({ error: "forbidden_host" });
    }
  });

  it("rejects names that are not DNS names", () => {
    for (const host of ["", "a..b", "-a.com", "a-.com", "a b.com", "a_b.com", "x".repeat(64) + ".com"]) {
      expect(target(host), host).toEqual({ error: "bad_request" });
    }
  });

  it("limits targets to ALLOWED_HOSTS when set, with wildcard subdomains", () => {
    const policy: Policy = { ports: parsePorts(undefined), hosts: parseList("smp4.simplex.im, *.relays.example.org") };
    expect(target("smp4.simplex.im", 5223, policy)).toEqual({ host: "smp4.simplex.im" });
    expect(target("a.relays.example.org", 5223, policy)).toEqual({ host: "a.relays.example.org" });
    expect(target("relays.example.org", 5223, policy)).toEqual({ error: "forbidden_host" });
    expect(target("evilrelays.example.org", 5223, policy)).toEqual({ error: "forbidden_host" });
    expect(target("smp5.simplex.im", 5223, policy)).toEqual({ error: "forbidden_host" });
  });
});

describe("parseIpv6", () => {
  it("expands :: and embedded IPv4", () => {
    expect(parseIpv6("::1")).toEqual([0, 0, 0, 0, 0, 0, 0, 1]);
    expect(parseIpv6("1::")).toEqual([1, 0, 0, 0, 0, 0, 0, 0]);
    expect(parseIpv6("::ffff:1.2.3.4")).toEqual([0, 0, 0, 0, 0, 0xffff, 0x0102, 0x0304]);
    expect(parseIpv6("1:2:3:4:5:6:7")).toBeNull();
  });
});

describe("tokenEquals", () => {
  it("is true only for an identical token", () => {
    expect(tokenEquals("s3cret-token", "s3cret-token")).toBe(true);
    expect(tokenEquals("s3cret-tokeN", "s3cret-token")).toBe(false);
    expect(tokenEquals("s3cret", "s3cret-token")).toBe(false);
    expect(tokenEquals("s3cret-token-and-more", "s3cret-token")).toBe(false);
    expect(tokenEquals("", "s3cret-token")).toBe(false);
    expect(tokenEquals("ключ", "ключ")).toBe(true);
  });

  it("does not stop at the first differing byte", () => {
    // Count comparisons through a proxy over the given bytes: all of them are read either way.
    const expected = "a".repeat(64);
    const reads = (given: string) => {
      let n = 0;
      const enc = TextEncoder.prototype.encode;
      TextEncoder.prototype.encode = function (s?: string) {
        const bytes = enc.call(this, s);
        if (s !== given) return bytes;
        return new Proxy(bytes, {
          get(t, k) {
            if (typeof k === "string" && /^\d+$/.test(k)) n++;
            return Reflect.get(t, k);
          },
        });
      };
      try {
        tokenEquals(given, expected);
      } finally {
        TextEncoder.prototype.encode = enc;
      }
      return n;
    };
    expect(reads("b" + "a".repeat(63))).toBe(reads("a".repeat(63) + "b"));
    expect(reads("b" + "a".repeat(63))).toBe(64);
  });
});

describe("originAllowed", () => {
  it("allows any origin when ALLOWED_ORIGINS is empty", () => {
    expect(originAllowed(null, [])).toBe(true);
    expect(originAllowed("https://x.example", [])).toBe(true);
  });

  it("allows only listed origins when set", () => {
    const list = parseList("https://chat.example.org");
    expect(originAllowed("https://chat.example.org", list)).toBe(true);
    expect(originAllowed("https://evil.example", list)).toBe(false);
    expect(originAllowed(null, list)).toBe(false);
  });
});
