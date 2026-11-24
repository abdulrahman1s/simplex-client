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

`scripts/build-js.sh` builds the same into `pkg/web` (and the Node.js and Bun build into
`pkg/node`).
The `wasm-bindgen` CLI must match the crate's `wasm-bindgen` version (0.2.130).

## Run

Serve the folder over HTTP (modules do not load from `file://`):

```sh
python3 -m http.server -d examples/browser 8000
```

Open <http://localhost:8000>, enter a tunnel URL (the free public one, `wss://simplex-tunnel.citizen6775.workers.dev/v1`, or your own), a name and a
passphrase, and press Start. The same passphrase opens the saved store next time; a wrong one
is refused.

## The JS API

```js
import init, { Client } from "./pkg/simplex_client.js";

await init();

const client = await Client.start({
  name: "Alice",
  key: "passphrase",
  tunnel: "wss://simplex-tunnel.citizen6775.workers.dev/v1", // or transport: "tcp" on Node.js and Bun
  relays: [],             // optional smp:// addresses; the presets when empty
  snapshot: savedBytes,   // optional Uint8Array from client.save()
  tunnelToken: undefined, // optional, when the Worker sets TUNNEL_TOKEN
  timeout: 60000,         // optional, ms a waiting call waits
  log: false,             // optional, the engine's log on the console
});

client.on("ready", (me) => console.log(`Logged in as ${me.name}`));
client.on("request", (request) => request.accept());
client.on("friendAdd", (friend) => friend.send("welcome!"));

client.on("message", async (message) => {
  if (message.mine) return;
  await message.reply(message.text);
});
```

Events: `ready` (your profile), `message` and `messageUpdate` (a `Message`), `request` (a
`FriendRequest`, once), `friendAdd` and `friendRemove` (a `Friend`), `groupAdd` and
`groupRemove` (a `Group`), `groupInvite` (a `GroupInvite`), `memberAdd` (a `Member`),
`offline` and `online`, `change` (the store changed: save it) and `raw` (every engine event as
`{t, c}`). `on`, `off` and `once` take them; `await client.once("ready")` works too.

The objects act on their own, and every method returns a Promise:

- `Message`: `reply`, `react`, `unreact`, `edit`, `delete`; `text`, `mine`, `at`, `replyTo`,
  `chat` (a `Friend` or `Group`), `author` (a `Friend`, a `Member`, or `null` for yours), `raw`.
- `FriendRequest`: `accept`, `reject`; `id`, `name`.
- `Friend`: `send`, `remove`, `showCode`, `history({limit, before})`, `markRead`; `id`, `name`,
  `ready`, `verified`, `raw`.
- `Group`: `send`, `addMember(friend)`, `leave`, `members`, `history`, `markRead`; `id`,
  `name`, `membership`, `owner`, `raw`.
- `GroupInvite`: `accept`, `reject`; `group`, `name`. `Member`: `name`, `id`, `group`, `friend`.

On the client: `friends` and `groups` (`Map`s by id, from the last chat list), `user`,
`createGroup(name)` (resolves to the `Group`), `createAddress`, `createInvite`,
`deleteAddress`, `connect(link)`, `setName`, `request` (any engine request as its JSON),
`save` and `close`. A failure rejects with `{code, detail}`.

Files (XFTP) are not available in the browser build.
