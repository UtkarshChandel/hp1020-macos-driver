#!/usr/bin/env bash
# Build the installer package from source and install it (same result as
# double-clicking the .pkg from the GitHub release). Needs Rust: https://rustup.rs
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
[ "$(uname)" = "Darwin" ] || { echo "macOS only." >&2; exit 1; }
command -v cargo >/dev/null || { echo "Rust required: https://rustup.rs" >&2; exit 1; }
rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null 2>&1 || true

echo "==> Building the installer package..."
packaging/build-pkg.sh >/dev/null
PKG="$(ls -t dist/HP-LaserJet-1020-Driver-*.pkg | head -1)"

echo "==> Installing $PKG (needs your password)..."
sudo installer -pkg "$PKG" -target /
tail -n 3 /Library/Printers/hp1020/install.log
echo "Done. Print to 'HP LaserJet 1020' from any app (Cmd-P)."
