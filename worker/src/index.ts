// simplex-tunnel: a WebSocket to TCP byte pipe so simplex-client in a browser can reach SMP and
// XFTP relays. TLS runs end to end in the client; this Worker only sees ciphertext.
import type { Env } from "./env";
import { ipKey } from "./ipkey";
import { readLimits } from "./limits";
import { startTunnel } from "./tunnel";
import { originAllowed, parseList, parsePorts } from "./validate";

export { TunnelCounter } from "./counter";

const INFO = `simplex-tunnel

A WebSocket to TCP pipe for simplex-client running in a browser.
It forwards TLS ciphertext to SimpleX relays and nothing else.

Protocol: wss://<this host>/v1
Source (AGPL-3.0-only): https://github.com/abdulrahman1s/simplex-client/tree/main/worker
`;

export default {
  async fetch(request, env, ctx): Promise<Response> {
    const { pathname } = new URL(request.url);
    if (pathname === "/") return text(INFO, 200);
    if (pathname !== "/v1") return text("not found\n", 404);
    if (request.method !== "GET" || request.headers.get("Upgrade")?.toLowerCase() !== "websocket") {
      return text("expected a WebSocket upgrade\n", 426);
    }
    if (!originAllowed(request.headers.get("Origin"), parseList(env.ALLOWED_ORIGINS))) {
      return text("forbidden\n", 403);
    }

    const clientKey = await ipKey(request.headers.get("CF-Connecting-IP") ?? "", env.IP_SALT);
    if (env.TUNNEL_RL) {
      const { success } = await env.TUNNEL_RL.limit({ key: clientKey }).catch(() => ({ success: true }));
      if (!success) return text("rate_limited\n", 429);
    }

    const [client, server] = Object.values(new WebSocketPair());
    startTunnel(server, {
      env,
      limits: readLimits(env),
      policy: { ports: parsePorts(env.ALLOWED_PORTS), hosts: parseList(env.ALLOWED_HOSTS) },
      clientKey,
      waitUntil: (p) => ctx.waitUntil(p),
    });
    return new Response(null, { status: 101, webSocket: client });
  },
} satisfies ExportedHandler<Env>;

function text(body: string, status: number): Response {
  return new Response(body, { status, headers: { "content-type": "text/plain; charset=utf-8" } });
}
