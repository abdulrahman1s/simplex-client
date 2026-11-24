#!/usr/bin/env bash
# Builds the JS packages: pkg/web (browsers, bundlers) and pkg/node (Node.js 22+ and Bun).
# Needs the wasm32-unknown-unknown target and wasm-bindgen-cli 0.2.130 on PATH, and uses
# binaryen's wasm-opt when it is there.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --lib --profile wasm-release --target wasm32-unknown-unknown --no-default-features --features wasm
wasm="${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/wasm-release/simplex_client.wasm"

rm -rf pkg/web pkg/node
wasm-bindgen --target web --out-dir pkg/web "$wasm"
wasm-bindgen --target nodejs --out-dir pkg/node "$wasm"
# wasm-opt (binaryen) makes the module a fifth smaller and some calls a tenth faster. An
# install from git may not have it, so it is optional unless REQUIRE_WASM_OPT is set, as in CI.
if command -v wasm-opt > /dev/null; then
  for dir in pkg/web pkg/node; do
    wasm-opt -O3 "$dir/simplex_client_bg.wasm" -o "$dir/simplex_client_bg.wasm"
  done
elif [ -n "${REQUIRE_WASM_OPT:-}" ]; then
  echo "wasm-opt not found; install binaryen" >&2
  exit 1
else
  echo "warning: wasm-opt not found, so the packages are not optimised; install binaryen" >&2
fi
# Each build says what kind of module it is, whatever the package around it says.
echo '{ "type": "module" }' > pkg/web/package.json
echo '{ "type": "commonjs" }' > pkg/node/package.json
echo "built pkg/web and pkg/node"
