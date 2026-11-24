# simplex-client in a browser tab

A minimal chat page on the wasm build: it shows your address, lets you accept requests, lists
your friends and their messages, and has a send box. The store lives in memory and is saved to
IndexedDB, sealed under your passphrase, a second after each change.

Browsers cannot open TCP connections, so the client reaches SMP relays through a WebSocket
tunnel: the Worker in [`worker/`](../../worker). TLS runs inside wasm and is pinned by the
relay's fingerprint, so the tunnel only ever sees ciphertext.

## Build

From the repository root, either with `wasm-bindgen`:

```sh
cargo build --lib --release --target wasm32-unknown-unknown --no-default-features --features wasm
wasm-bindgen --target web --out-dir examples/browser/pkg \
  target/wasm32-unknown-unknown/release/simplex_client.wasm
```

or with `wasm-pack`:

```sh
wasm-pack build --target web --out-dir examples/browser/pkg -- --no-default-features --features wasm
```

`scripts/build-js.sh` builds the same into `pkg/web` (and the Node build into `pkg/node`).
The `wasm-bindgen` CLI must match the crate's `wasm-bindgen` version (0.2.130).

## Run

Serve the folder over HTTP (modules do not load from `file://`):

```sh
python3 -m http.server -d examples/browser 8000
```

Open <http://localhost:8000>, enter your tunnel URL (`wss://<worker>/v1`), a name and a
passphrase, and press Start. The same passphrase opens the saved store next time; a wrong one
is refused.

## The JS API

```js
import init, { Client } from "./pkg/simplex_client.js";
await init();
const client = await Client.start({
  name: "Alice",
  key: "passphrase",
  tunnel: "wss://simplex-tunnel.<account>.workers.dev/v1", // or transport: "tcp" on Node
  relays: [],             // optional smp:// addresses; the presets when empty
  snapshot: savedBytes,   // optional Uint8Array from client.save()
  tunnelToken: undefined, // optional, when the Worker sets TUNNEL_TOKEN
  timeout: 60000,         // optional, ms a waiting call waits
  log: false,             // optional, the engine's log on the console
});
client.on("message", (msg) => { if (!msg.mine) client.reply(msg, msg.text); });
client.on("request", (req) => client.accept(req.id));
client.on("event", (ev) => console.log(ev)); // every engine event, {t, c}
client.on("change", () => save(client.save()));
const link = await client.createAddress();
const invite = await client.createInvite();
client.connect(link);
client.send({ friend: 1 }, "hi");
const bytes = client.save();
client.close();
```

Named events: `event` (every engine event), `message` (each new message, with `mine` and
`text`), `request` (each new contact request, once), `ready` (your profile), `offline`
(`true` while no relay is reachable), `chats` (friends, groups, requests, previews) and
`change` (the store changed; time to save).

A chat is `{friend: id}` or `{group: id}`; the `{t: "direct", c: id}` form an event carries
works too. Methods: `send`, `reply`, `edit`, `delete`, `react`, `unreact`, `accept`, `reject`,
`removeFriend`, `connect`, `createInvite`, `createAddress`, `deleteAddress`, `newGroup`,
`addMember`, `joinGroup`, `leaveGroup`, `members`, `history`, `markRead`, `showCode`,
`setName`, `profile`, `request` (any engine request as its JSON), `save`, `close`, `on`, `off`.
`createInvite`, `createAddress`, `history`, `members` and `showCode` return Promises; a failure
rejects, and every method throws, with `{code, detail}`.

Files (XFTP) are not available in the browser build.
