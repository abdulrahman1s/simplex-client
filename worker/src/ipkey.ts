// The only form in which a client's IP is used: a salted SHA-256, as the rate-limit key and
// the counter's name. The raw address is never stored or logged.
import { parseIpv6 } from "./validate";

export async function ipKey(ip: string, salt: string | undefined): Promise<string> {
  const data = new TextEncoder().encode(`${salt ?? ""}\u0000${clientNetwork(ip)}`);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", data));
  return [...digest.subarray(0, 16)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** An IPv6 client usually holds a whole /64 and could rotate through it, so that is the unit. */
export function clientNetwork(ip: string): string {
  const groups = ip.includes(":") ? parseIpv6(ip.toLowerCase()) : null;
  return groups ? `${groups.slice(0, 4).map((g) => g.toString(16)).join(":")}::/64` : ip;
}
