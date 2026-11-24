#!/usr/bin/env bash
# Builds the JS packages: pkg/web (browsers, bundlers) and pkg/node (Node.js 22+ and Bun).
# Needs the wasm32-unknown-unknown target and wasm-bindgen-cli 0.2.130 on PATH.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --lib --release --target wasm32-unknown-unknown --no-default-features --features wasm
wasm="${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/release/simplex_client.wasm"

rm -rf pkg/web pkg/node
wasm-bindgen --target web --out-dir pkg/web "$wasm"
wasm-bindgen --target nodejs --out-dir pkg/node "$wasm"
# Each build says what kind of module it is, whatever the package around it says.
echo '{ "type": "module" }' > pkg/web/package.json
echo '{ "type": "commonjs" }' > pkg/node/package.json
echo "built pkg/web and pkg/node"
