// Resolves a relay name to addresses over DNS-over-HTTPS (RFC 8484), so the Worker dials and
// range-checks the exact address. The name travels in a POST body, never in a URL.

const RESOLVER = "https://cloudflare-dns.com/dns-query";
const RESOLVE_TIMEOUT_MS = 3000;
export const TYPE_A = 1;
export const TYPE_AAAA = 28;
const TYPE_LEN: Record<number, number> = { [TYPE_A]: 4, [TYPE_AAAA]: 16 };

/** IPv4 addresses first, then IPv6; empty when the name has neither or the lookup fails. */
export async function resolve(name: string, fetcher: typeof fetch = fetch): Promise<string[]> {
  for (const type of [TYPE_A, TYPE_AAAA]) {
    const res = await fetcher(RESOLVER, {
      method: "POST",
      headers: { "content-type": "application/dns-message", accept: "application/dns-message" },
      body: buildQuery(name, type),
      signal: AbortSignal.timeout(RESOLVE_TIMEOUT_MS),
    });
    if (!res.ok) return [];
    const found = parseAnswers(new Uint8Array(await res.arrayBuffer()), type);
    if (found.length > 0) return found;
  }
  return [];
}

export function buildQuery(name: string, type: number): Uint8Array {
  const out = [0, 0, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]; // id 0, RD, one question
  for (const label of name.split(".")) {
    const bytes = new TextEncoder().encode(label);
    out.push(bytes.length, ...bytes);
  }
  out.push(0, type >> 8, type & 0xff, 0, 1);
  return Uint8Array.from(out);
}

/** The answer records of `type` in a response, as address strings. Malformed input gives []. */
export function parseAnswers(msg: Uint8Array, type: number): string[] {
  try {
    const u16 = (p: number) => {
      if (p + 2 > msg.length) throw new RangeError("truncated");
      return (msg[p] << 8) | msg[p + 1];
    };
    if (msg.length < 12 || (msg[2] & 0x80) === 0 || (msg[3] & 0x0f) !== 0) return [];
    const questions = u16(4);
    const answers = u16(6);
    let p = 12;
    for (let i = 0; i < questions; i++) p = skipName(msg, p) + 4;
    const out: string[] = [];
    for (let i = 0; i < answers; i++) {
      p = skipName(msg, p);
      const rtype = u16(p);
      const len = u16(p + 8);
      const data = p + 10;
      if (data + len > msg.length) return [];
      if (rtype === type && len === TYPE_LEN[type]) out.push(formatAddress(msg.subarray(data, data + len)));
      p = data + len;
    }
    return out;
  } catch {
    return [];
  }
}

/** Skips a possibly compressed name and returns the offset after it. */
function skipName(msg: Uint8Array, p: number): number {
  for (;;) {
    if (p >= msg.length) throw new RangeError("truncated");
    const len = msg[p];
    if (len === 0) return p + 1;
    if ((len & 0xc0) === 0xc0) return p + 2;
    if (len > 63) throw new RangeError("bad label");
    p += 1 + len;
  }
}

function formatAddress(b: Uint8Array): string {
  if (b.length === 4) return [...b].join(".");
  const groups: string[] = [];
  for (let i = 0; i < 16; i += 2) groups.push(((b[i] << 8) | b[i + 1]).toString(16));
  return groups.join(":");
}
