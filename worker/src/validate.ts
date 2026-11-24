// Checks on the first frame: its shape, the target host and port, and the optional token.

export type ErrorCode =
  | "bad_request"
  | "forbidden_port"
  | "forbidden_host"
  | "unauthorized"
  | "not_tls"
  | "connect_failed"
  | "idle"
  | "rate_limited"
  | "too_many_tunnels"
  | "quota";

export interface Hello {
  host: string;
  port: number;
  token?: string;
}

export interface Policy {
  ports: ReadonlySet<number>;
  /** Exact names or `*.example.com` patterns; empty allows any public host. */
  hosts: readonly string[];
}

const MAX_HELLO_TEXT = 1024;

export function parseList(value: string | undefined): string[] {
  return (value ?? "")
    .split(",")
    .map((s) => s.trim().toLowerCase())
    .filter((s) => s !== "");
}

export function parsePorts(value: string | undefined): Set<number> {
  return new Set(
    parseList(value ?? "5223,443")
      .map(Number)
      .filter((p) => Number.isInteger(p) && p >= 1 && p <= 65535),
  );
}

export function parseHello(text: string): Hello | null {
  if (text.length > MAX_HELLO_TEXT) return null;
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch {
    return null;
  }
  if (typeof v !== "object" || v === null || Array.isArray(v)) return null;
  const { host, port, token } = v as Record<string, unknown>;
  if (typeof host !== "string" || typeof port !== "number") return null;
  if (!Number.isInteger(port) || port < 1 || port > 65535) return null;
  if (token !== undefined && typeof token !== "string") return null;
  return token === undefined ? { host, port } : { host, port, token };
}

/** Returns the host to dial (lower case, no brackets, no trailing dot), or why it is refused. */
export function checkTarget(
  rawHost: string,
  port: number,
  policy: Policy,
): { host: string } | { error: ErrorCode } {
  if (!policy.ports.has(port)) return { error: "forbidden_port" };
  let host = rawHost.toLowerCase();
  if (host.startsWith("[") && host.endsWith("]")) host = host.slice(1, -1);
  if (host.endsWith(".")) host = host.slice(0, -1);

  const error = host.includes(":") ? checkIpv6(host) : /^[0-9.]+$/.test(host) ? checkIpv4(host) : checkName(host);
  if (error) return { error };
  if (policy.hosts.length > 0 && !policy.hosts.some((p) => hostMatches(host, p))) {
    return { error: "forbidden_host" };
  }
  return { host };
}

/** `connect()` wants IPv6 literals in brackets; a bare one is refused. */
export function socketHostname(host: string): string {
  return host.includes(":") ? `[${host}]` : host;
}

export function hostMatches(host: string, pattern: string): boolean {
  return pattern.startsWith("*.") ? host.endsWith(pattern.slice(1)) : host === pattern;
}

const LABEL = /^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/;
const BLOCKED_SUFFIXES = [".local", ".internal", ".localhost"];

function checkName(host: string): ErrorCode | null {
  const labels = host.split(".");
  if (host.length > 253 || !labels.every((l) => LABEL.test(l))) return "bad_request";
  // Relays have public names. One label (`localhost`, `metadata`) or a numeric last label
  // (`0x7f.0.0.1`, which some resolvers read as an address) is not one.
  if (labels.length < 2 || /^[0-9]+$/.test(labels[labels.length - 1])) return "forbidden_host";
  if (BLOCKED_SUFFIXES.some((s) => host.endsWith(s))) return "forbidden_host";
  return null;
}

// Non-public IPv4 ranges as [network, prefix bits].
const BLOCKED_V4: ReadonlyArray<[number, number]> = [
  [ip4(0, 0, 0, 0), 8], // "this network"
  [ip4(10, 0, 0, 0), 8], // private
  [ip4(100, 64, 0, 0), 10], // CGNAT
  [ip4(127, 0, 0, 0), 8], // loopback
  [ip4(169, 254, 0, 0), 16], // link-local, cloud metadata
  [ip4(172, 16, 0, 0), 12], // private
  [ip4(192, 0, 0, 0), 24], // IETF protocol assignments
  [ip4(192, 168, 0, 0), 16], // private
  [ip4(198, 18, 0, 0), 15], // benchmarking
  [ip4(224, 0, 0, 0), 4], // multicast
  [ip4(240, 0, 0, 0), 4], // reserved, broadcast
];

function ip4(a: number, b: number, c: number, d: number): number {
  return ((a << 24) | (b << 16) | (c << 8) | d) >>> 0;
}

/** Only canonical dotted quads: `127.1`, `2130706433` and `010.0.0.1` are refused outright. */
export function parseIpv4(s: string): number | null {
  const parts = s.split(".");
  if (parts.length !== 4 || !parts.every((p) => /^(0|[1-9][0-9]{0,2})$/.test(p) && Number(p) <= 255)) {
    return null;
  }
  const [a, b, c, d] = parts.map(Number);
  return ip4(a, b, c, d);
}

function v4Blocked(addr: number): boolean {
  return BLOCKED_V4.some(([net, bits]) => addr >>> (32 - bits) === net >>> (32 - bits));
}

function checkIpv4(host: string): ErrorCode | null {
  const addr = parseIpv4(host);
  return addr === null || v4Blocked(addr) ? "forbidden_host" : null;
}

/** Eight 16-bit groups, or null. Zone ids (`%eth0`) are not accepted. */
export function parseIpv6(s: string): number[] | null {
  if (!/^[0-9a-f:.]+$/.test(s)) return null;
  const halves = s.split("::");
  if (halves.length > 2) return null;
  const groups = (part: string): number[] | null => {
    if (part === "") return [];
    const out: number[] = [];
    const items = part.split(":");
    for (const [i, item] of items.entries()) {
      if (i === items.length - 1 && item.includes(".")) {
        const v4 = parseIpv4(item);
        if (v4 === null) return null;
        out.push(v4 >>> 16, v4 & 0xffff);
      } else if (/^[0-9a-f]{1,4}$/.test(item)) {
        out.push(parseInt(item, 16));
      } else {
        return null;
      }
    }
    return out;
  };
  const head = groups(halves[0]);
  const tail = halves.length === 2 ? groups(halves[1]) : [];
  if (head === null || tail === null) return null;
  if (halves.length === 1) return head.length === 8 ? head : null;
  if (head.length + tail.length > 7) return null;
  return [...head, ...new Array<number>(8 - head.length - tail.length).fill(0), ...tail];
}

function checkIpv6(host: string): ErrorCode | null {
  const g = parseIpv6(host);
  if (g === null) return "bad_request";
  const embedded = (hi: number, lo: number) => v4Blocked(((hi << 16) | lo) >>> 0);
  const zeros = (n: number) => g.slice(0, n).every((x) => x === 0);
  const blocked =
    (zeros(5) && (g[5] === 0 || (g[5] === 0xffff && embedded(g[6], g[7])))) || // ::, ::1, v4-compatible, v4-mapped
    (g[0] === 0x64 && g[1] === 0xff9b && (g[2] !== 0 || embedded(g[6], g[7]))) || // NAT64
    (g[0] === 0x0100 && g[1] === 0 && g[2] === 0 && g[3] === 0) || // discard-only 100::/64
    (g[0] === 0x2001 && g[1] === 0x0db8) || // documentation
    (g[0] === 0x2002 && embedded(g[1], g[2])) || // 6to4
    (g[0] & 0xfe00) === 0xfc00 || // unique local
    (g[0] & 0xffc0) === 0xfe80 || // link-local
    (g[0] & 0xffc0) === 0xfec0 || // site-local
    (g[0] & 0xff00) === 0xff00; // multicast
  return blocked ? "forbidden_host" : null;
}

/** Compares in time that depends only on the expected token's length. */
export function tokenEquals(given: string, expected: string): boolean {
  const enc = new TextEncoder();
  const a = enc.encode(given);
  const b = enc.encode(expected);
  let diff = a.length ^ b.length;
  for (let i = 0; i < b.length; i++) diff |= (a[i] | 0) ^ b[i];
  return diff === 0;
}

export function originAllowed(origin: string | null, allowed: readonly string[]): boolean {
  return allowed.length === 0 || (origin !== null && allowed.includes(origin.toLowerCase()));
}
