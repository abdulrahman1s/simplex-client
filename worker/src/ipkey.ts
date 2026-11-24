// The only form in which a client's IP is used: a salted SHA-256, as the rate-limit key and
// the counter's name. The raw address is never stored or logged.

export async function ipKey(ip: string, salt: string | undefined): Promise<string> {
  const data = new TextEncoder().encode(`${salt ?? ""}\u0000${ip}`);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", data));
  return [...digest.subarray(0, 16)].map((b) => b.toString(16).padStart(2, "0")).join("");
}
