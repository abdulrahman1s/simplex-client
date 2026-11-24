// The open-proxy guard over the whole TLS handshake. Until the relay sends its first encrypted
// record (type 23), every record the client sends must be a SimpleX ClientHello (the first one
// and any after a HelloRetryRequest) or a one-byte compatibility ChangeCipherSpec, and every
// record the relay sends must be a TLS record.
import { isSimplexHello, MAX_HELLO_RECORD } from "./clienthello";

const CHANGE_CIPHER_SPEC = 20;
const ALERT = 21;
const HANDSHAKE = 22;
const APPLICATION_DATA = 23;
const HEADER = 5;

export type ClientRecords = { verdict: "ok"; used: number } | { verdict: "not_tls" };

/** The record at the start of `buf`: its type and length, "more" for a partial header. */
function header(buf: Uint8Array, types: readonly number[]): { type: number; len: number } | "more" | "not_tls" {
  if (buf.length >= 1 && !types.includes(buf[0])) return "not_tls";
  if (buf.length >= 2 && buf[1] !== 3) return "not_tls";
  if (buf.length >= 3 && (buf[2] < 1 || buf[2] > 3)) return "not_tls";
  if (buf.length < HEADER) return "more";
  const len = (buf[3] << 8) | buf[4];
  const max = buf[0] === APPLICATION_DATA ? MAX_HELLO_RECORD + 256 : MAX_HELLO_RECORD;
  return len === 0 || len > max ? "not_tls" : { type: buf[0], len };
}

/**
 * Checks each whole client record at the start of `buf`. `used` is how many bytes they span;
 * what follows is one incomplete record. `first` means none has passed, so it must be a hello.
 */
export function checkClientRecords(buf: Uint8Array, first: boolean): ClientRecords {
  let used = 0;
  for (;;) {
    const rest = buf.subarray(used);
    const h = header(rest, [HANDSHAKE, CHANGE_CIPHER_SPEC]);
    if (h === "not_tls") return { verdict: "not_tls" };
    if (h === "more" || rest.length < HEADER + h.len) return { verdict: "ok", used };
    const body = rest.subarray(HEADER, HEADER + h.len);
    const fine = h.type === HANDSHAKE ? isSimplexHello(body) : !first && h.len === 1 && body[0] === 1;
    if (!fine) return { verdict: "not_tls" };
    first = false;
    used += HEADER + h.len;
  }
}

export type ServerVerdict = "handshake" | "encrypted" | "not_tls";

/** Follows the relay's record headers across reads until its first encrypted record. */
export class ServerRecords {
  private head = new Uint8Array(HEADER);
  private have = 0;
  private left = 0;

  feed(bytes: Uint8Array): ServerVerdict {
    let i = 0;
    while (i < bytes.length) {
      if (this.left > 0) {
        const n = Math.min(this.left, bytes.length - i);
        this.left -= n;
        i += n;
        continue;
      }
      this.head[this.have++] = bytes[i++];
      const h = header(this.head.subarray(0, this.have), [CHANGE_CIPHER_SPEC, ALERT, HANDSHAKE, APPLICATION_DATA]);
      if (h === "not_tls") return "not_tls";
      if (h === "more") continue;
      if (h.type === APPLICATION_DATA) return "encrypted";
      this.have = 0;
      this.left = h.len;
    }
    return "handshake";
  }
}
