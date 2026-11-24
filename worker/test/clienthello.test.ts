import { createServer, type AddressInfo } from "node:net";
import { connect as tlsConnect } from "node:tls";
import { describe, expect, it } from "vitest";
import { checkClientHello, MAX_HELLO_RECORD } from "../src/clienthello";
import { extension, extensionTypes, rustClientHello } from "./fixtures";

/** The first bytes node's TLS client (OpenSSL) writes, captured on a local socket. */
async function captureNodeHello(options: { ALPNProtocols: string[]; servername?: string }): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const server = createServer((sock) => {
      sock.once("data", (data) => {
        resolve(new Uint8Array(data));
        sock.destroy();
        server.close();
      });
    });
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address() as AddressInfo;
      const tls = tlsConnect({ host: "127.0.0.1", port, rejectUnauthorized: false, ...options });
      tls.on("error", () => undefined);
    });
    server.on("error", reject);
  });
}

describe("checkClientHello", () => {
  it("accepts simplex-client's ClientHello offering smp/1 without SNI", () => {
    expect(checkClientHello(rustClientHello())).toBe("ok");
  });

  it("accepts the xftp/1 ALPN", () => {
    expect(checkClientHello(rustClientHello({ alpn: ["xftp/1"] }))).toBe("ok");
  });

  it("rejects a ClientHello that names a server with SNI", () => {
    expect(checkClientHello(rustClientHello({ sni: "smp4.simplex.im" }))).toBe("not_tls");
  });

  it("rejects a ClientHello with encrypted client hello", () => {
    expect(checkClientHello(rustClientHello({ extraExtensions: [extension(0xfe0d, [0])] }))).toBe("not_tls");
  });

  it("rejects a ClientHello without ALPN", () => {
    expect(checkClientHello(rustClientHello({ alpn: [] }))).toBe("not_tls");
  });

  it("rejects web ALPNs, alone or next to smp/1", () => {
    expect(checkClientHello(rustClientHello({ alpn: ["h2"] }))).toBe("not_tls");
    expect(checkClientHello(rustClientHello({ alpn: ["h2", "smp/1"] }))).toBe("not_tls");
  });

  it("rejects a duplicated extension", () => {
    const dup = extension(43, [2, 3, 4]);
    expect(checkClientHello(rustClientHello({ extraExtensions: [dup] }))).toBe("not_tls");
  });

  it("rejects an HTTP request", () => {
    const http = new TextEncoder().encode("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n");
    expect(checkClientHello(http)).toBe("not_tls");
  });

  it("rejects non-TLS bytes from the first byte", () => {
    expect(checkClientHello(Uint8Array.of(0x53, 0x53, 0x48))).toBe("not_tls");
    expect(checkClientHello(Uint8Array.of(0x17))).toBe("not_tls");
  });

  it("rejects a bad record version and an oversized record length", () => {
    expect(checkClientHello(Uint8Array.of(0x16, 2, 0))).toBe("not_tls");
    const len = MAX_HELLO_RECORD + 1;
    expect(checkClientHello(Uint8Array.of(0x16, 3, 1, len >> 8, len & 0xff))).toBe("not_tls");
  });

  it("asks for more until the whole first record has arrived", () => {
    const hello = rustClientHello();
    for (const n of [0, 1, 4, 5, 40, hello.length - 1]) {
      expect(checkClientHello(hello.subarray(0, n))).toBe("more");
    }
    expect(checkClientHello(hello)).toBe("ok");
  });

  it("accepts bytes that follow the first record", () => {
    const hello = rustClientHello();
    const more = new Uint8Array(hello.length + 3);
    more.set(hello);
    expect(checkClientHello(more)).toBe("ok");
  });

  it("rejects a record truncated inside the handshake message", () => {
    const hello = rustClientHello();
    const cut = hello.slice(0, hello.length - 10);
    cut[3] = (cut.length - 5) >> 8;
    cut[4] = (cut.length - 5) & 0xff;
    expect(checkClientHello(cut)).toBe("not_tls");
  });

  it("accepts OpenSSL's ClientHello with smp/1 and no SNI", async () => {
    const hello = await captureNodeHello({ ALPNProtocols: ["smp/1"] });
    expect(extensionTypes(hello)).not.toContain(0);
    expect(checkClientHello(hello)).toBe("ok");
  });

  it("rejects OpenSSL's ClientHello once it carries SNI", async () => {
    const hello = await captureNodeHello({ ALPNProtocols: ["smp/1"], servername: "smp4.simplex.im" });
    expect(extensionTypes(hello)).toContain(0);
    expect(checkClientHello(hello)).toBe("not_tls");
  });
});
