# AGENTS.md

Instructions for coding agents and contributors.

## Repo map

| Path | Job |
|---|---|
| `src/encoding`, `src/types`, `src/crypto` | wire encoding, shared types, primitives |
| `src/tls`, `src/h2` | TCP + TLS transport and HTTP/2 (native only) |
| `src/smp` | SMP commands, queues, link and envelope codecs |
| `src/agent`, `src/ratchet` | connections, double ratchet with PQ KEM |
| `src/chat` | chat message codec, contacts, groups, files, calls |
| `src/xftp` | file crypto, descriptions, transfer |
| `src/store` | SQLite store sealed with a passphrase |
| `src/engine` | threads, subscriptions, the event stream |
| `src/client` | high-level `Client`, `Event`, `Request` |
| `src/wasm.rs` | wasm-bindgen bindings (feature `wasm`) |
| `tests/` | integration and golden tests |

## The gate

All of these must pass before a change is done:

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo clippy --target wasm32-unknown-unknown --no-default-features --features wasm --all-targets -- -D warnings
cargo test --target wasm32-unknown-unknown --no-default-features --features wasm
cargo deny check
```

Use `nix develop -c ...` if Rust is not installed. The wasm tests need `wasm-bindgen-cli`
0.2.130 and node. Live interop tests are `#[ignore]`d and need `SIMPLEX_TEST_SMP` and
`SIMPLEX_TEST_XFTP` pointing at local servers.

## Style

- One `//!` line per module. Comments say why, never what.
- Files under about 150 lines, one action per file. `mod.rs` holds only `mod` and `pub use`.
- Handlers read top to bottom: look up, check, change, save, notify. Use `?` and `let ... else`.
- No `unwrap` or `expect` outside tests.
- Match exhaustively on our own enums; no `_ =>`.
- No async. Use threads and crossbeam channels.
- Time comes through the `Clock` seam. Clippy bans `SystemTime::now` and `Instant::now`.
- `#![forbid(unsafe_code)]`.
- Every growing collection documents its eviction rule next to it.
- Test names state the rule (`ratchet_rejects_replayed_message_key`), not the function.
- Native and wasm must both pass clippy with `-D warnings`.

## Adding a request to the Client

1. Add the variant to `Request` (and its result, if any) in `src/client`.
2. Add one handler file under `src/engine` that does the work and returns `Result<()>`;
   register it in the engine's dispatch table.
3. Add a thin method on `Client` that builds the request and waits for the reply.
4. Add an `Event` variant only if the outcome is something the caller did not ask for.
5. Add a test that fails without the change.

## Port from pinned sources

Protocol code is ported from simplex-chat v7.1.4 and simplexmq commit `fdbcb24d`. Read those
sources; do not write protocol details from memory. Keep byte-exact golden tests for every
encoding, and cite the upstream module in the file's `//!` line. Do not follow newer upstream
versions without bumping the pin on purpose, in its own commit.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `refactor:`,
`perf:`, `docs:`, `test:`, `ci:`, `chore:`, with an optional scope. Name files explicitly in
`git add`. Record user-visible changes in `CHANGELOG.md` under Unreleased. Commit only when
asked.
