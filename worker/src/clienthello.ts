// The open-proxy guard: the client's first TLS record must be a ClientHello that offers only
// the SimpleX ALPNs (smp/1, xftp/1) and names no server (no SNI, no ECH), as simplex-client sends.

/** The largest first record we buffer: a TLS plaintext record body is at most 2^14 bytes. */
export const MAX_HELLO_RECORD = 16384;
export const MAX_HELLO_BYTES = 5 + MAX_HELLO_RECORD;

const ALLOWED_ALPNS = new Set(["smp/1", "xftp/1"]);
const RECORD_HANDSHAKE = 0x16;
const CLIENT_HELLO = 1;
const EXT_SNI = 0;
const EXT_ALPN = 16;
const EXT_ECH = 0xfe0d;

export type HelloCheck = "more" | "ok" | "not_tls";

/** Checks the bytes received so far. "more" means the first record is not complete yet. */
export function checkClientHello(buf: Uint8Array): HelloCheck {
  if (buf.length >= 1 && buf[0] !== RECORD_HANDSHAKE) return "not_tls";
  if (buf.length >= 2 && buf[1] !== 3) return "not_tls";
  if (buf.length >= 3 && (buf[2] < 1 || buf[2] > 3)) return "not_tls";
  if (buf.length < 5) return "more";
  const len = (buf[3] << 8) | buf[4];
  if (len < 4 || len > MAX_HELLO_RECORD) return "not_tls";
  if (buf.length < 5 + len) return "more";
  try {
    return helloIsSimplex(buf.subarray(5, 5 + len)) ? "ok" : "not_tls";
  } catch {
    return "not_tls";
  }
}

function helloIsSimplex(record: Uint8Array): boolean {
  const r = new Reader(record);
  if (r.u8() !== CLIENT_HELLO) return false;
  // One whole ClientHello in the first record; simplex-client never fragments it.
  const body = new Reader(r.take(r.u24()));
  if (!r.done) return false;
  body.take(2 + 32); // legacy_version, random
  if (body.vec8().length > 32) return false;
  const suites = body.vec16();
  if (suites.length === 0 || suites.length % 2 !== 0) return false;
  if (body.vec8().length === 0) return false;
  const exts = new Reader(body.vec16());
  if (!body.done) return false;

  const seen = new Set<number>();
  let alpn = false;
  while (!exts.done) {
    const type = exts.u16();
    const data = exts.vec16();
    if (seen.has(type)) return false;
    seen.add(type);
    if (type === EXT_SNI || type === EXT_ECH) return false;
    if (type === EXT_ALPN) {
      alpn = alpnIsSimplex(data);
      if (!alpn) return false;
    }
  }
  return alpn;
}

function alpnIsSimplex(data: Uint8Array): boolean {
  const r = new Reader(data);
  const list = new Reader(r.vec16());
  if (!r.done || list.done) return false;
  while (!list.done) {
    if (!ALLOWED_ALPNS.has(new TextDecoder().decode(list.vec8()))) return false;
  }
  return true;
}

class Reader {
  private pos = 0;
  constructor(private readonly bytes: Uint8Array) {}

  get done(): boolean {
    return this.pos === this.bytes.length;
  }

  take(n: number): Uint8Array {
    if (this.pos + n > this.bytes.length) throw new RangeError("truncated");
    const out = this.bytes.subarray(this.pos, this.pos + n);
    this.pos += n;
    return out;
  }

  u8(): number {
    return this.take(1)[0];
  }

  u16(): number {
    const b = this.take(2);
    return (b[0] << 8) | b[1];
  }

  u24(): number {
    const b = this.take(3);
    return (b[0] << 16) | (b[1] << 8) | b[2];
  }

  vec8(): Uint8Array {
    return this.take(this.u8());
  }

  vec16(): Uint8Array {
    return this.take(this.u16());
  }
}
