# simplex-tunnel

A Cloudflare Worker that lets the browser (wasm) build of `simplex-client` reach SimpleX relays.
Browsers cannot open raw TCP connections, and SMP relays listen on TCP port 5223 (sometimes
443). This Worker is only a **WebSocket to TCP byte pipe**: the browser opens a WebSocket,
names a relay, and the Worker connects to it with `cloudflare:sockets` and copies bytes both
ways.

It is the transport piece only. The Rust side that speaks this protocol from wasm is separate
work in the main crate.

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/abdulrahman1s/simplex-client/tree/main/worker)

## Trust model

The client runs TLS 1.3 itself, inside wasm. It pins the relay by the certificate fingerprint
in the `smp://<fingerprint>@host` address. The Worker never holds a key and only ever sees TLS
ciphertext. If it tried to impersonate the relay, the client's fingerprint check would fail.
Under TLS, SMP encrypts per queue, and messages are end-to-end encrypted with the double
ratchet.

**What the Worker operator and Cloudflare can see:**

- your IP address;
- the relay's host name and port. The Worker also looks the name up with Cloudflare's DNS
  resolver (DNS over HTTPS, the name in the request body);
- when you connect, and the timing and size of the traffic. SMP sends fixed 16 KiB blocks, so
  sizes say little beyond "a block went by".

**What they cannot see:**

- message content, files or calls;
- your keys or your contacts;
- which queues you use;
- anything inside TLS. TLS terminates in your browser, not at Cloudflare, and is pinned by the
  relay's fingerprint.

The Worker logs nothing. It has no `console.log`, and `wrangler.toml` turns observability,
invocation logs and Logpush off. The only thing it stores is a salted hash of your IP (of your
/64 for IPv6), with the times of your recent and open tunnels. That record is the per-IP
counter (see [Limits](#limits)). It is deleted once you have no open tunnels and none started
in the last minute. If a tunnel ends without telling the counter, it is deleted at most
30 minutes later. Cloudflare as a
company still handles the connection, so it can see the metadata above whatever this code
does. If that matters to you, deploy your own Worker, or put it behind a network you trust.

## Protocol (v1)

1. Open a WebSocket to `wss://<worker-host>/v1`. Any other path answers 404, and `/` a short
   plain-text page. No query string is needed.
2. Send the first frame as **text** JSON: `{"host":"smp4.simplex.im","port":5223}`. Add
   `"token":"..."` when the deployment sets `TUNNEL_TOKEN`. The target is in a frame, not the
   URL, so it never appears in request URLs or logs.
3. The Worker validates the target and connects with
   `connect({hostname, port}, {secureTransport: "off", allowHalfOpen: false})`.
   - When the socket opens, it sends the text frame `{"ok":true}`.
   - On any failure, it sends `{"ok":false,"error":"<code>"}` and closes with code 1008
     (policy) or 1011 (upstream).
4. After that, **binary** frames in both directions carry raw TCP bytes. Your first binary
   bytes must be a TLS ClientHello that offers ALPN `smp/1` or `xftp/1` and has no SNI, and
   so must every record you send until the relay's first encrypted record (see below). A text
   frame after the first one closes the tunnel.
5. Closing either side closes the other. When the relay closes, the WebSocket closes with
   1000. A relay error closes it with 1011 `upstream`. Reaching the maximum lifetime closes it
   with 1000 `lifetime`; reconnect.

One tunnel is one TCP socket.

| Error | Close | Meaning |
|---|---|---|
| `bad_request` | 1008 | The first frame is not valid JSON of the right shape; binary before it, a text frame after it, an oversized frame, or no first frame within `HELLO_TIMEOUT_SECONDS` |
| `forbidden_port` | 1008 | The port is not in `ALLOWED_PORTS` |
| `forbidden_host` | 1008 | A host that is neither a preset relay nor in `ALLOWED_HOSTS`; or, under `ALLOWED_HOSTS=*`, a private, loopback, link-local, CGNAT or otherwise non-public address or a local name |
| `unauthorized` | 1008 | The token is missing or wrong |
| `not_tls` | 1008 | A record before the relay's first encrypted record is not a SimpleX ClientHello or a compatibility ChangeCipherSpec, or the ClientHello did not arrive within `HELLO_TIMEOUT_SECONDS` of `{"ok":true}` |
| `connect_failed` | 1011 | The relay's name did not resolve, or the relay could not be reached within 10 s |
| `idle` | 1008 | No bytes either way for `IDLE_TIMEOUT_SECONDS` |
| `too_many_tunnels` | 1008 | This IP already has `MAX_TUNNELS_PER_IP` tunnels open |
| `quota` | 1008 | The tunnel used up `MAX_BYTES_PER_TUNNEL` |
| `rate_limited` | 1008, or HTTP 429 | More than `MAX_NEW_TUNNELS_PER_MINUTE` new tunnels from this IP, or the limits could not be checked. The fast first gate answers the upgrade with HTTP 429 before any WebSocket exists, so the browser only sees a failed connection |

## Not an open proxy

- **Ports.** Only `ALLOWED_PORTS` (default `5223,443`).
- **Hosts.**
  - By default only the preset SimpleX relays are allowed: the clearnet names of the SMP
    relays in `src/engine/presets.rs` and the XFTP servers in `src/xftp/servers.rs`
    (`smpN.simplex.im`, `smpN.simplexonflux.com`, `xftpN.simplex.im`,
    `xftpN.simplexonflux.com`). A test keeps the list equal to the Rust presets.
  - `ALLOWED_HOSTS` adds exact names or `*.example.com` patterns. `ALLOWED_HOSTS=*` allows
    any public host; the checks below still apply to every host.
  - IP literals in private, loopback, link-local, CGNAT, multicast and other non-public ranges
    are refused, for both IPv4 and IPv6. This includes IPv4 embedded in IPv6 (mapped, NAT64,
    6to4).
  - Non-canonical numeric forms (`127.1`, `2130706433`, `0x7f.0.0.1`) are refused.
  - Single-label names and names ending in `.local`, `.internal` or `.localhost` are refused.
  - Names are resolved by the Worker over DNS over HTTPS (IPv4 first, then IPv6), and the
    resolved address goes through the same range checks. The Worker dials exactly that
    address, so a public name pointing at a private network is refused as well.
- **TLS check.**
  - Until the relay sends its first encrypted record (type 23), every record the client sends
    must be a ClientHello offering only `smp/1` or `xftp/1` as ALPN, with no `server_name`
    (SNI) and no encrypted-client-hello extension, or a one-byte compatibility
    ChangeCipherSpec. The first record must be a ClientHello, and a second one after a
    HelloRetryRequest is checked like the first. SimpleX clients contact relays without SNI.
  - The Worker holds back a record until it has all of it (at most 16 KiB), parses only what
    it needs, and forwards each record once it passes. Bytes after a record are checked as the
    next record.
  - A server that sends bytes before the ClientHello, or answers with anything but TLS
    records, closes the tunnel.
  - This keeps out HTTP, SSH and other protocols. It does not stop a client that controls a
    server on an allowed host from speaking anything once that server sends one encrypted
    record, which is why the host allowlist is the real limit. Web servers that accept a
    ClientHello without SNI may be reachable under `ALLOWED_HOSTS=*`.
- **Token.** When the `TUNNEL_TOKEN` secret is set, the first frame must carry the same
  `token`. It is compared in constant time.
- **Origins.** When `ALLOWED_ORIGINS` is set, the upgrade's `Origin` header must be one of
  them.

## Limits

| Limit | Setting | Default |
|---|---|---|
| New tunnels per IP, exact | `MAX_NEW_TUNNELS_PER_MINUTE` | 30 |
| New tunnels per IP, fast first gate | `[[ratelimits]]` `TUNNEL_RL` in `wrangler.toml`: `simple = { limit, period }`, where period is 10 or 60 s | 30 per 60 s |
| Open tunnels per IP | `MAX_TUNNELS_PER_IP` | 8 |
| Time to send the first frame, and the ClientHello after `{"ok":true}` | `HELLO_TIMEOUT_SECONDS` | 10 |
| Idle time with no bytes either way | `IDLE_TIMEOUT_SECONDS` | 900 (15 min). SMP pings every ~30 s keep a live tunnel open |
| Tunnel lifetime | `MAX_LIFETIME_SECONDS` | 86400 (24 h) |
| Bytes per tunnel, both directions | `MAX_BYTES_PER_TUNNEL` | 1073741824 (1 GiB) |
| WebSocket frame size | `MAX_FRAME_BYTES` | 65536 (64 KiB). Larger frames from the client close the tunnel, and relay bytes are split to fit |
| Client bytes waiting for the relay socket | fixed | 1 MiB. Writes to the relay are awaited in order, and a client that runs this far ahead is closed |
| Relay name lookup | fixed | 3 s |
| Relay connect time | fixed | 10 s |

- **How IPs are used.** The client IP is hashed: SHA-256 over the `IP_SALT` secret and the IP,
  or over the IP alone if no salt is set. For IPv6 the /64 prefix is hashed, since one client
  usually holds a whole /64. The hash is the key for both the rate-limit binding
  and the per-IP counter.
- **The fast first gate.** The Workers Rate Limiting binding refuses an upgrade before the
  WebSocket opens. Its counters live on each Cloudflare machine and are synced loosely, so a
  client spread over many machines can get past it. Measured on the deployment, 90 upgrades in
  under a minute all passed it.
- **The per-IP counter.** This enforces both per-IP limits exactly.
  - It is a SQLite-backed Durable Object (works on the Free plan), named by the hash.
  - It counts the tunnels started in the last minute, and holds one lease per open tunnel.
  - Each open tunnel renews its lease every 10 minutes, and a lease lasts 30 minutes. If a
    release is lost, for example because the Worker was evicted, the slot frees itself within
    30 minutes.
  - The counter deletes all of its storage, the hash included, once it has no open tunnels
    and no starts in the last minute.
- **Failing closed.** If the counter or the rate limiter fails, the tunnel is refused with
  `rate_limited`, so an outage there cannot turn the limits off. A deployment without the
  `TUNNELS` or `TUNNEL_RL` binding has no such limit.

Values that are missing, malformed or not positive fall back to the default. On the Workers
Free plan, each tunnel costs one request against the daily request allowance and one DNS
lookup subrequest. It also makes Durable Object calls: one to admit, one to release, and six
renewals per hour while it stays open.

## Configuration

`[vars]` in `wrangler.toml`:

| Variable | Default | Meaning |
|---|---|---|
| `ALLOWED_PORTS` | `5223,443` | Ports a tunnel may connect to |
| `ALLOWED_HOSTS` | empty (the preset relays only) | Comma list of exact names or `*.example.com` patterns allowed besides the preset relays; `*` allows any public host |
| `ALLOWED_ORIGINS` | empty (any) | Comma list of allowed `Origin` values, such as `https://chat.example.org` |
| `MAX_TUNNELS_PER_IP`, `MAX_NEW_TUNNELS_PER_MINUTE`, `HELLO_TIMEOUT_SECONDS`, `IDLE_TIMEOUT_SECONDS`, `MAX_LIFETIME_SECONDS`, `MAX_BYTES_PER_TUNNEL`, `MAX_FRAME_BYTES` | see [Limits](#limits) | |

Secrets, set with `npx wrangler secret put <NAME>`:

| Secret | Meaning |
|---|---|
| `TUNNEL_TOKEN` | When set, every tunnel's first frame must carry this `token` |
| `IP_SALT` | Salt for hashing client IPs. Set a random value so the hashes can't be matched against a list of IPs |

## Deploy your own

```sh
cd worker
npm ci
npx wrangler login
npx wrangler deploy
```

Optional:

```sh
npx wrangler secret put IP_SALT       # recommended: any long random string
npx wrangler secret put TUNNEL_TOKEN  # only your clients can use the tunnel
```

`wrangler deploy` prints the `https://simplex-tunnel.<your-subdomain>.workers.dev` URL. Point
the client at `wss://simplex-tunnel.<your-subdomain>.workers.dev/v1`. You can also use the
**Deploy to Cloudflare** button above, which forks the repository and deploys this folder.

Notes for the Workers Free plan:

- CPU time is limited per request (10 ms on Free), and waiting on the network does not count.
  The Worker does little work per byte: it copies buffers and parses one ClientHello.
  Cloudflare does not document whether CPU time adds up over a WebSocket that stays open for
  hours, and this has not been tested. A busy tunnel on the Free plan might be cut off early.
- Open connections have no wall-clock limit while the client stays connected.
- Cloudflare restarts long connections when it updates its runtime (with up to 20 minutes'
  notice), so the client must reconnect, as it does after `lifetime`.
- `connect()` cannot reach Cloudflare's own IP ranges or port 25.

## Development

```sh
npm ci
npx tsc --noEmit   # type check
npx vitest run     # unit tests: ClientHello check, host/port/token rules, DNS, limits, leases
npx wrangler dev   # local Worker on http://localhost:8787
```

## License

AGPL-3.0-only, like the rest of the repository. If you run a modified version for other
people, the AGPL requires you to offer them its source. The Worker's `/` page links to this
source.
