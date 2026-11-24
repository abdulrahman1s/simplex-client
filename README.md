# simplex-client

A SimpleX Chat protocol client in Rust: connect, chat, run groups and send files through a
small, plain API.

[![CI](https://github.com/abdulrahman1s/simplex-client/actions/workflows/ci.yml/badge.svg)](https://github.com/abdulrahman1s/simplex-client/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](LICENSE)

> **Unofficial and unaudited.** This project is not affiliated with or endorsed by SimpleX
> Chat Ltd. It has not had a security review. Do not rely on it where a flaw would hurt someone.

## Features

- SMP queues over TLS, with the agent's double ratchet and the post-quantum KEM
- Chat messages, contacts, groups, replies, edits, deletes and reactions
- XFTP file upload and download, with file crypto and descriptions
- Calls (signalling and frame crypto)
- SQLite store sealed with a passphrase
- A threaded engine and a high-level `Client`; no async runtime
- A `wasm32-unknown-unknown` build of the pure protocol core
- Low-level modules stay public if you need to go below `Client`

## Quick start

The crate is not published on crates.io. Depend on it through git:

```toml
[dependencies]
simplex-client = { git = "https://github.com/abdulrahman1s/simplex-client" }
```

An echo bot:

```rust
use simplex_client::{Client, Event};

fn main() -> simplex_client::Result<()> {
    let client = Client::builder("./bot-data").name("Echo Bot").key("store passphrase").start()?;
    println!("my address: {}", client.create_address()?);
    for event in client.events() {
        match event {
            Event::Messages(msgs) => for m in msgs { if !m.mine() { client.reply(&m, &m.text())?; } }
            Event::Chats { requests, .. } => for r in requests { client.accept(r.id)?; }
            _ => {}
        }
    }
    Ok(())
}
```

Requests on `Client`: `send`, `reply`, `edit`, `delete`, `react`, `accept`, `reject`,
`connect`, `create_invite`, `create_address`, `new_group`, `add_member`, `join_group`,
`leave_group`, `send_file`, `receive_file`, `history`, `mark_read`. `request(Request)` is the
escape hatch for anything else.

### Events

| Event | Fires when |
|---|---|
| `Event::Messages` | new messages arrive, or your own are echoed back |
| `Event::Chats` | contacts, groups or incoming contact requests change |
| `Event::Files` | a file transfer starts, progresses, finishes or fails |
| `Event::Calls` | a call is offered, answered or ended |

The exact variants are listed in the crate docs (`cargo doc --open`).

## WebAssembly

The `wasm` feature builds the pure protocol core for `wasm32-unknown-unknown`: encoding,
crypto, the double ratchet, the chat message codec, SMP and agent link and envelope codecs,
XFTP file crypto and descriptions, and small wasm-bindgen bindings (`src/wasm.rs`).

**The browser cannot open SMP connections yet.** There is no raw TCP there, and the
connection manager is threaded. Wasm gives you the codecs and crypto, not a working client.

```sh
cargo build --target wasm32-unknown-unknown --no-default-features --features wasm
```

`getrandom` 0.3 needs its JS backend on wasm. This repository sets it in `.cargo/config.toml`;
projects that depend on the crate must set it too:

```sh
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build --target wasm32-unknown-unknown ...
```

Sketch, after running `wasm-bindgen` on the output (see `src/wasm.rs` for the exported
functions):

```js
import init, * as simplex from "./pkg/simplex_client.js";
await init();
console.log(Object.keys(simplex)); // the exported codecs and crypto helpers
```

## Architecture

```
 Client            high-level API: builder, events, requests
   |
 engine            threads, subscriptions, retries, the event stream
   |
 chat              chat messages, contacts, groups, files, calls
   |
 agent (ratchet)   connections, double ratchet with PQ KEM
   |
 smp               SMP commands and queues
   |
 tls (h2)          TCP + TLS transport

 xftp              file chunks, crypto and descriptions (over h2), used by chat for files
 store             SQLite store, sealed with a passphrase
 crypto, encoding, types   shared by every layer
```

## Feature flags

| Feature | Default | What it enables |
|---|---|---|
| `native` | yes | Threads, TCP and TLS, the SQLite store, the engine and `Client` |
| `wasm` | no | The pure protocol core and wasm-bindgen bindings; use with `--no-default-features` |

## Protocol versions tracked

Ported from simplex-chat v7.1.4 and simplexmq commit `fdbcb24d`. Newer upstream changes are
not followed automatically.

## Development

```sh
nix develop                    # pinned toolchain (1.99.1, wasm32 target)
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo clippy --target wasm32-unknown-unknown --no-default-features --features wasm --all-targets -- -D warnings
cargo test --target wasm32-unknown-unknown --no-default-features --features wasm
cargo deny check
```

Wasm tests need `wasm-bindgen-cli` 0.2.130 (`wasm-bindgen-test-runner`) and node. The MSRV is
Rust 1.95. See [AGENTS.md](AGENTS.md) for the code style.

Live interop tests are `#[ignore]`d. They need a local `smp-server` and `xftp-server`:

```sh
export SIMPLEX_TEST_SMP='smp://<fingerprint>@127.0.0.1:<port>'
export SIMPLEX_TEST_XFTP='xftp://<fingerprint>@127.0.0.1:<port>'
cargo test -- --ignored
```

## License

AGPL-3.0-only, see [LICENSE](LICENSE). The code is ported from SimpleX Chat and simplexmq,
which are AGPL-3.0, so this port carries the same license. If you run a modified version as a
network service, the AGPL requires you to offer its source to your users.

## Credits

The protocol and the original implementation are the work of
[SimpleX Chat](https://github.com/simplex-chat). This crate would not exist without
[simplex-chat](https://github.com/simplex-chat/simplex-chat) and
[simplexmq](https://github.com/simplex-chat/simplexmq).
