import { describe, expect, it } from "vitest";
import { askCounter, firstGate } from "../src/admission";
import { checkClientRecords, ServerRecords } from "../src/handshake";
import { rustClientHello } from "./fixtures";

const cat = (...parts: Uint8Array[]) => Uint8Array.from(parts.flatMap((p) => [...p]));
const text = (s: string) => new TextEncoder().encode(s);
const record = (type: number, body: number[]) => Uint8Array.of(type, 3, 3, body.length >> 8, body.length & 0xff, ...body);
const ccs = record(20, [1]);

describe("client records before the relay's first encrypted record", () => {
  it("pass whole SimpleX hellos and keep an incomplete record back", () => {
    const hello = rustClientHello();
    expect(checkClientRecords(hello, true)).toEqual({ verdict: "ok", used: hello.length });
    expect(checkClientRecords(cat(hello, hello.subarray(0, 40)), true)).toEqual({ verdict: "ok", used: hello.length });
    expect(checkClientRecords(hello.subarray(0, 40), true)).toEqual({ verdict: "ok", used: 0 });
  });

  it("refuse anything after the hello that is not a TLS record, in the same frame or later", () => {
    const get = text("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n");
    expect(checkClientRecords(cat(rustClientHello(), get), true)).toEqual({ verdict: "not_tls" });
    expect(checkClientRecords(get, false)).toEqual({ verdict: "not_tls" });
  });

  it("check a second hello after a HelloRetryRequest as strictly as the first", () => {
    const hello = rustClientHello();
    const named = rustClientHello({ sni: "example.com" });
    expect(checkClientRecords(cat(hello, ccs, hello), true).verdict).toBe("ok");
    expect(checkClientRecords(cat(hello, ccs, named), true)).toEqual({ verdict: "not_tls" });
    expect(checkClientRecords(named, false)).toEqual({ verdict: "not_tls" });
  });

  it("refuse a ChangeCipherSpec first or of another shape, and application data", () => {
    expect(checkClientRecords(ccs, true)).toEqual({ verdict: "not_tls" });
    expect(checkClientRecords(record(20, [2]), false)).toEqual({ verdict: "not_tls" });
    expect(checkClientRecords(record(23, [1, 2, 3]), false)).toEqual({ verdict: "not_tls" });
    expect(checkClientRecords(record(21, [2, 40]), false)).toEqual({ verdict: "not_tls" });
  });
});

describe("relay records", () => {
  it("open the tunnel only at the relay's first encrypted record, across reads", () => {
    const s = new ServerRecords();
    const hello = record(22, new Array(90).fill(2));
    expect(s.feed(hello.subarray(0, 3))).toBe("handshake");
    expect(s.feed(cat(hello.subarray(3), ccs))).toBe("handshake");
    expect(s.feed(Uint8Array.of(23, 3, 3))).toBe("handshake");
    expect(s.feed(Uint8Array.of(0, 40))).toBe("encrypted");
  });

  it("refuse a relay that does not answer in TLS records", () => {
    expect(new ServerRecords().feed(text("HTTP/1.1 400 Bad Request\r\n"))).toBe("not_tls");
    expect(new ServerRecords().feed(cat(record(22, [1]), text("SSH-2.0")))).toBe("not_tls");
  });
});

describe("limits fail closed", () => {
  it("refuse the upgrade when the rate limiter errors", async () => {
    expect(await firstGate(() => Promise.reject(new Error("down")))).toBe(false);
    expect(await firstGate(async () => ({ success: true }))).toBe(true);
    expect(await firstGate(async () => ({ success: false }))).toBe(false);
  });

  it("refuse the tunnel when the counter errors, and still give back a lease it may have taken", async () => {
    expect(await askCounter(() => Promise.reject(new Error("down")))).toEqual({ admission: "rate_limited", heldLease: true });
    expect(await askCounter(async () => "ok")).toEqual({ admission: "ok", heldLease: true });
    expect(await askCounter(async () => "too_many_tunnels")).toEqual({ admission: "too_many_tunnels", heldLease: false });
  });
});
