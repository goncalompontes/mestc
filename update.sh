#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

echo "=== Building mestc (compiler) ==="
cargo install --path crates/mestc --locked

echo "=== Building mest-lsp (language server) ==="
cargo install --path crates/mest-lsp --locked

echo "=== Done ==="
echo "Binaries installed to ~/.cargo/bin/mest and ~/.cargo/bin/mest-lsp"
