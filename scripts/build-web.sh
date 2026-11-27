#!/usr/bin/env bash
# Build the Lagom compiler + interpreter for the browser.
#
# Output: web/pkg/lagom_web.js and lagom_web_bg.wasm (ES module, --target web).
# Requirements:
#   - rustup target add wasm32-unknown-unknown
#   - cargo install wasm-bindgen-cli --version 0.2.129   (must match the
#     wasm-bindgen version pinned in crates/lagom_web/Cargo.toml)
set -euo pipefail

cd "$(dirname "$0")/.."

REQUIRED_BINDGEN="0.2.129"
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "wasm-bindgen CLI not found. Install it with:" >&2
  echo "  cargo install wasm-bindgen-cli --version $REQUIRED_BINDGEN" >&2
  exit 1
fi
found="$(wasm-bindgen --version | awk '{print $2}')"
if [ "$found" != "$REQUIRED_BINDGEN" ]; then
  echo "wasm-bindgen CLI is $found but the crate needs $REQUIRED_BINDGEN." >&2
  echo "  cargo install wasm-bindgen-cli --version $REQUIRED_BINDGEN --force" >&2
  exit 1
fi

cargo build -p lagom_web --target wasm32-unknown-unknown --release

wasm-bindgen \
  --target web \
  --out-dir web/pkg \
  --out-name lagom_web \
  target/wasm32-unknown-unknown/release/lagom_web.wasm

echo "Built web/pkg/lagom_web.js and web/pkg/lagom_web_bg.wasm"
