// One WebSocket, one TCP socket: tunnel protocol v1 (README.md, "Protocol").
// Frame handling is serialized on one promise chain so state changes and writes stay in order.
import { connect } from "cloudflare:sockets";
import { checkClientHello, MAX_HELLO_BYTES } from "./clienthello";
import type { Env } from "./env";
import { Meter, type Limits } from "./limits";
import { checkTarget, parseHello, socketHostname, tokenEquals, type ErrorCode, type Policy } from "./validate";

const CONNECT_TIMEOUT_MS = 10_000;
const CONNECT_ATTEMPTS = 4;
const TIMEOUT = new Error("timeout");
/** How long a lost release keeps a lease, beyond the tunnel's own lifetime. */
const LEASE_SLACK_MS = 60_000;

type Phase = "hello" | "connecting" | "probe" | "open" | "closed";

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
  private probe: Uint8Array | null = null;
  private probeLen = 0;
  private socket?: Socket;
  private writer?: WritableStreamDefaultWriter<Uint8Array>;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private release?: () => Promise<void>;
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
    if (!(await this.acquire())) return this.fail("too_many_tunnels");
    if (this.closed()) return;

    const socket = await this.dial(socketHostname(target.host), hello.port);
    if (this.closed()) return;
    if (!socket) return this.fail("connect_failed");
    this.writer = socket.writable.getWriter();
    this.phase = "probe";
    this.ws.send(JSON.stringify({ ok: true }));
    this.arm(this.cx.limits.helloTimeoutMs); // the ClientHello must follow promptly
    void this.pump(socket);
  }

  /**
   * Opens the relay socket. Cloudflare sometimes refuses a connect to a host name at once
   * ("proxy request failed") and accepts the next try, so quick refusals are retried.
   */
  private async dial(hostname: string, port: number): Promise<Socket | null> {
    for (let attempt = 1; attempt <= CONNECT_ATTEMPTS && !this.closed(); attempt++) {
      const socket = connect({ hostname, port }, { secureTransport: "off", allowHalfOpen: false });
      this.socket = socket;
      socket.closed.catch(() => undefined);
      try {
        await withTimeout(socket.opened, CONNECT_TIMEOUT_MS);
        return socket;
      } catch (e) {
        socket.close().catch(() => undefined);
        if (e === TIMEOUT) return null;
      }
      await new Promise((r) => setTimeout(r, 100 * attempt));
    }
    return null;
  }

  /** Takes a lease on the client's counter. Fails open: an unreachable counter must not stop chat. */
  private async acquire(): Promise<boolean> {
    const ns = this.cx.env.TUNNELS;
    if (!ns) return true;
    const stub = ns.getByName(this.cx.clientKey);
    const id = crypto.randomUUID();
    this.release = () => stub.release(id);
    const { maxTunnelsPerIp, maxLifetimeMs } = this.cx.limits;
    try {
      const ok = await stub.acquire(id, maxTunnelsPerIp, maxLifetimeMs + LEASE_SLACK_MS);
      if (!ok) this.release = undefined;
      return ok;
    } catch {
      return true;
    }
  }

  private async forward(bytes: Uint8Array): Promise<void> {
    this.queued -= bytes.byteLength;
    const writer = this.writer;
    if (!writer) return;
    if (this.phase !== "probe") return writer.write(bytes);

    this.probe ??= new Uint8Array(MAX_HELLO_BYTES + this.cx.limits.maxFrameBytes);
    this.probe.set(bytes, this.probeLen);
    this.probeLen += bytes.byteLength;
    const buffered = this.probe.subarray(0, this.probeLen);
    const verdict = checkClientHello(buffered);
    if (verdict === "more") return;
    if (verdict === "not_tls") return this.fail("not_tls");
    this.phase = "open";
    this.probe = null;
    this.arm(0);
    return writer.write(buffered);
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
    timer = setTimeout(() => reject(TIMEOUT), ms);
  });
  return Promise.race([p, timeout]).finally(() => clearTimeout(timer));
}
