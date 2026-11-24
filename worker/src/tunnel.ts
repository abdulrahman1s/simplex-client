// One WebSocket, one TCP socket: tunnel protocol v1 (README.md, "Protocol").
// Frame handling is serialized on one promise chain so state changes and writes stay in order.
import { connect } from "cloudflare:sockets";
import { askCounter } from "./admission";
import { MAX_HELLO_BYTES } from "./clienthello";
import type { Env } from "./env";
import { checkClientRecords, ServerRecords } from "./handshake";
import { Meter, type Limits } from "./limits";
import { resolve } from "./dns";
import type { Admission } from "./leases";
import {
  checkAddress,
  checkTarget,
  isIpLiteral,
  parseHello,
  socketHostname,
  tokenEquals,
  type ErrorCode,
  type Policy,
} from "./validate";

const CONNECT_TIMEOUT_MS = 10_000;
/** An open tunnel renews its lease; one that stops renewing (a lost release) frees its slot. */
const LEASE_MS = 30 * 60_000;
const LEASE_RENEW_MS = 10 * 60_000;

/** `probe`: waiting for the ClientHello; `handshake`: until the relay's first encrypted record. */
type Phase = "hello" | "connecting" | "probe" | "handshake" | "open" | "closed";
const EMPTY = new Uint8Array(0);

export interface TunnelContext {
  env: Env;
  limits: Limits;
  policy: Policy;
  /** The hashed client IP. */
  clientKey: string;
  waitUntil(p: Promise<unknown>): void;
}

export function startTunnel(ws: WebSocket, cx: TunnelContext): void {
  new Tunnel(ws, cx);
}

class Tunnel {
  private phase: Phase = "hello";
  private chain: Promise<void> = Promise.resolve();
  private queued = 0;
  /** Client bytes of a record not yet whole, held back until it is checked. */
  private pending = EMPTY;
  private readonly server = new ServerRecords();
  private socket?: Socket;
  private writer?: WritableStreamDefaultWriter<Uint8Array>;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private release?: () => Promise<void>;
  private renewer: ReturnType<typeof setInterval> | null = null;
  private readonly meter: Meter;

  constructor(
    private readonly ws: WebSocket,
    private readonly cx: TunnelContext,
  ) {
    this.meter = new Meter(cx.limits, Date.now());
    // Newer compatibility dates deliver binary frames as Blob unless asked otherwise.
    ws.binaryType = "arraybuffer";
    ws.addEventListener("message", (e) => this.onMessage(e.data));
    ws.addEventListener("close", () => this.end(1000, "closed"));
    ws.addEventListener("error", () => this.end(1011, "error"));
    ws.accept();
    this.arm(cx.limits.helloTimeoutMs);
  }

  private onMessage(data: string | ArrayBuffer): void {
    if (this.phase === "closed") return;
    if (typeof data === "string") {
      if (this.phase !== "hello") return this.fail("bad_request");
      this.phase = "connecting";
      this.enqueue(() => this.open(data));
      return;
    }
    if (this.phase === "hello" || !(data instanceof ArrayBuffer)) return this.fail("bad_request");
    const limits = this.cx.limits;
    if (data.byteLength > limits.maxFrameBytes) return this.fail("bad_request");
    if (!this.meter.add(data.byteLength, Date.now())) return this.fail("quota");
    this.queued += data.byteLength;
    if (this.queued > limits.maxQueuedBytes) return this.fail("bad_request");
    const bytes = new Uint8Array(data);
    this.enqueue(() => this.forward(bytes));
  }

  private enqueue(step: () => Promise<void>): void {
    this.chain = this.chain
      .then(() => (this.phase === "closed" ? undefined : step()))
      .catch(() => this.end(1011, "upstream"));
  }

  private async open(text: string): Promise<void> {
    const hello = parseHello(text);
    if (!hello) return this.fail("bad_request");
    const target = checkTarget(hello.host, hello.port, this.cx.policy);
    if ("error" in target) return this.fail(target.error);
    const token = this.cx.env.TUNNEL_TOKEN;
    if (token && !tokenEquals(hello.token ?? "", token)) return this.fail("unauthorized");
    const admission = await this.admit();
    if (admission !== "ok") return this.fail(admission);
    const address = await resolveAddress(target.host);
    if (this.closed()) return;
    if (address === null) return this.fail("connect_failed");
    if (checkAddress(address)) return this.fail("forbidden_host");

    const socket = connect(
      { hostname: socketHostname(address), port: hello.port },
      { secureTransport: "off", allowHalfOpen: false },
    );
    this.socket = socket;
    socket.closed.catch(() => undefined);
    try {
      await withTimeout(socket.opened, CONNECT_TIMEOUT_MS);
    } catch {
      return this.fail("connect_failed");
    }
    if (this.closed()) return;
    this.writer = socket.writable.getWriter();
    this.phase = "probe";
    this.ws.send(JSON.stringify({ ok: true }));
    this.arm(this.cx.limits.helloTimeoutMs); // the ClientHello must follow promptly
    void this.pump(socket);
  }

  /** Asks the client's counter to admit this tunnel; a counter that errors refuses it. */
  private async admit(): Promise<Admission> {
    const ns = this.cx.env.TUNNELS;
    if (!ns) return "ok";
    // A fresh stub per call: a stub that has thrown once may stay broken.
    const stub = () => ns.getByName(this.cx.clientKey);
    const id = crypto.randomUUID();
    this.release = () => stub().release(id);
    const { maxTunnelsPerIp, maxNewTunnelsPerMinute } = this.cx.limits;
    const rules = { maxOpen: maxTunnelsPerIp, maxStarts: maxNewTunnelsPerMinute, leaseMs: LEASE_MS };
    const { admission, heldLease } = await askCounter(() => stub().admit(id, rules));
    if (!heldLease) this.release = undefined;
    if (admission !== "ok") return admission;
    this.renewer = setInterval(() => void stub().renew(id, LEASE_MS).catch(() => undefined), LEASE_RENEW_MS);
    return "ok";
  }

  private async forward(bytes: Uint8Array): Promise<void> {
    this.queued -= bytes.byteLength;
    const writer = this.writer;
    if (!writer) return;
    const buffered = this.pending.byteLength === 0 ? bytes : concat(this.pending, bytes);
    this.pending = EMPTY;
    if (this.phase === "open") return writer.write(buffered);

    const check = checkClientRecords(buffered, this.phase === "probe");
    if (check.verdict === "not_tls") return this.fail("not_tls");
    this.pending = buffered.slice(check.used);
    if (this.pending.byteLength > MAX_HELLO_BYTES) return this.fail("not_tls");
    if (check.used === 0) return;
    if (this.phase === "probe") {
      this.phase = "handshake";
      this.arm(this.cx.limits.helloTimeoutMs); // the relay must answer the hello promptly too
    }
    return writer.write(buffered.subarray(0, check.used));
  }

  private async pump(socket: Socket): Promise<void> {
    const reader = socket.readable.getReader();
    const max = this.cx.limits.maxFrameBytes;
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done || this.phase === "closed") break;
        // A relay speaks only after the ClientHello; a server that talks first is not one.
        if (this.phase === "probe") return this.end(1011, "upstream");
        if (this.phase === "handshake") {
          const verdict = this.server.feed(value);
          if (verdict === "not_tls") return this.end(1011, "upstream");
          if (verdict === "encrypted") {
            this.phase = "open";
            this.arm(0);
          }
        }
        if (!this.meter.add(value.byteLength, Date.now())) return this.fail("quota");
        for (let i = 0; i < value.byteLength; i += max) this.ws.send(value.subarray(i, i + max));
      }
      this.end(1000, "closed");
    } catch {
      this.end(1011, "upstream");
    }
  }

  /** Re-reads the phase, which can change while an await is pending. */
  private closed(): boolean {
    return this.phase === "closed";
  }

  private arm(ms: number): void {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.tick(), ms);
  }

  private tick(): void {
    if (this.phase === "hello") return this.fail("bad_request");
    if (this.phase === "probe") return this.fail("not_tls");
    if (this.phase === "handshake") return this.end(1011, "upstream");
    if (this.phase === "closed") return;
    const now = Date.now();
    const expired = this.meter.expired(now);
    if (expired === "idle") return this.fail("idle");
    if (expired === "lifetime") return this.end(1000, "lifetime");
    this.arm(Math.max(1000, this.meter.nextCheck(now)));
  }

  /** Tells the client why with `{"ok":false,"error":code}`, then closes. */
  private fail(code: ErrorCode): void {
    if (this.phase === "closed") return;
    try {
      this.ws.send(JSON.stringify({ ok: false, error: code }));
    } catch {
      // The client is already gone.
    }
    this.end(code === "connect_failed" ? 1011 : 1008, code);
  }

  private end(code: number, reason: string): void {
    if (this.phase === "closed") return;
    this.phase = "closed";
    clearTimeout(this.timer);
    clearInterval(this.renewer);
    try {
      this.ws.close(code, reason);
    } catch {
      // Already closing.
    }
    this.socket?.close().catch(() => undefined);
    if (this.release) {
      this.cx.waitUntil(this.release().catch(() => undefined));
      this.release = undefined;
    }
  }
}

function withTimeout<T>(p: Promise<T>, ms: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error("timeout")), ms);
  });
  return Promise.race([p, timeout]).finally(() => clearTimeout(timer));
}

/**
 * The address to dial: a literal as given, or a name's first resolved address. Dialling the
 * address we checked means a name that points at a private network is refused too.
 */
async function resolveAddress(host: string): Promise<string | null> {
  if (isIpLiteral(host)) return host;
  const addresses = await resolve(host).catch(() => []);
  return addresses[0] ?? null;
}

function concat(a: Uint8Array, b: Uint8Array): Uint8Array {
  const out = new Uint8Array(a.byteLength + b.byteLength);
  out.set(a);
  out.set(b, a.byteLength);
  return out;
}
