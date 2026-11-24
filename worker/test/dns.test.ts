import { describe, expect, it } from "vitest";
import { buildQuery, parseAnswers, resolve, TYPE_A, TYPE_AAAA } from "../src/dns";

const u16 = (n: number) => [n >> 8, n & 0xff];
const name = (s: string) => [...s.split(".").flatMap((l) => [l.length, ...new TextEncoder().encode(l)]), 0];
const rr = (owner: number[], type: number, data: number[]) => [...owner, ...u16(type), 0, 1, 0, 0, 0, 60, ...u16(data.length), ...data];

/** A response for smp.example.com: a CNAME to relay.example.net, then its A or AAAA records. */
function response(type: number, records: number[][], rcode = 0): Uint8Array {
  const question = [...name("smp.example.com"), ...u16(type), 0, 1];
  const cname = rr([0xc0, 12], 5, name("relay.example.net"));
  const answers = records.map((data) => rr([0xc0, 12 + question.length + 12], type, data));
  return Uint8Array.from([0, 0, 0x81, 0x80 | rcode, ...u16(1), ...u16(1 + answers.length), 0, 0, 0, 0, ...question, ...cname, ...answers.flat()]);
}

describe("buildQuery", () => {
  it("asks one recursive question for the name and type", () => {
    expect([...buildQuery("smp.example.com", TYPE_A)]).toEqual([0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, ...name("smp.example.com"), 0, 1, 0, 1]);
  });
});

describe("parseAnswers", () => {
  it("follows past a CNAME with compressed names to the A records", () => {
    expect(parseAnswers(response(TYPE_A, [[172, 236, 220, 91], [1, 1, 1, 1]]), TYPE_A)).toEqual(["172.236.220.91", "1.1.1.1"]);
  });

  it("formats AAAA records as full IPv6", () => {
    const v6 = [0x26, 0x00, 0x3c, 0x17, 0, 0, 0, 0, 0x20, 0, 0x13, 0xff, 0xfe, 0x29, 0x45, 0x9d];
    expect(parseAnswers(response(TYPE_AAAA, [v6]), TYPE_AAAA)).toEqual(["2600:3c17:0:0:2000:13ff:fe29:459d"]);
  });

  it("gives nothing for an error code, a truncated message or a query echoed back", () => {
    expect(parseAnswers(response(TYPE_A, [[1, 1, 1, 1]], 3), TYPE_A)).toEqual([]);
    const full = response(TYPE_A, [[1, 1, 1, 1]]);
    expect(parseAnswers(full.subarray(0, full.length - 2), TYPE_A)).toEqual([]);
    expect(parseAnswers(buildQuery("smp.example.com", TYPE_A), TYPE_A)).toEqual([]);
    expect(parseAnswers(new Uint8Array(5), TYPE_A)).toEqual([]);
  });

  it("skips records whose length does not fit the type", () => {
    expect(parseAnswers(response(TYPE_A, [[1, 1, 1]]), TYPE_A)).toEqual([]);
  });
});

describe("resolve", () => {
  const fake = (answers: Record<number, Uint8Array>, seen: { url: string; type: number }[] = []) =>
    (async (url: string, init: RequestInit) => {
      const q = init.body as Uint8Array;
      const type = (q[q.length - 4] << 8) | q[q.length - 3];
      seen.push({ url, type });
      return new Response(answers[type] ?? response(type, []));
    }) as unknown as typeof fetch;

  it("prefers IPv4 and posts the name in the body, not the URL", async () => {
    const seen: { url: string; type: number }[] = [];
    expect(await resolve("smp.example.com", fake({ [TYPE_A]: response(TYPE_A, [[1, 2, 3, 4]]) }, seen))).toEqual(["1.2.3.4"]);
    expect(seen).toEqual([{ url: "https://cloudflare-dns.com/dns-query", type: TYPE_A }]);
  });

  it("falls back to IPv6 when the name has no IPv4", async () => {
    const v6 = [0x2a, 0x01, 0x04, 0xf8, ...new Array(11).fill(0), 1];
    expect(await resolve("smp.example.com", fake({ [TYPE_AAAA]: response(TYPE_AAAA, [v6]) }))).toEqual(["2a01:4f8:0:0:0:0:0:1"]);
  });

  it("gives nothing when the resolver fails", async () => {
    const failing = (async () => new Response("", { status: 500 })) as unknown as typeof fetch;
    expect(await resolve("smp.example.com", failing)).toEqual([]);
  });
});
