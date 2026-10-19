#!/usr/bin/env bash
# Native macOS driver for the HP LaserJet 1018 / 1020 / 1020 Plus.
# Builds the Rust CUPS filter (rastertozjs) and USB backend (hp1020), fetches HP's
# firmware blob, and creates a normal Cmd-P printer queue. See README.md.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FW_DIR=/Library/Printers/hp1020/firmware
PPD_DIR=/Library/Printers/hp1020
QUEUE=HP_LaserJet_1020
PLUGIN_URL=https://www.openprinting.org/download/printdriver/auxfiles/HP/plugins
# sha256 of the firmware inside HPLIP's per-model plugin (same bytes in hplip 2.8 .. 3.26)
FW_1020_SHA=9a6d03c858d9cf64ba86fdbe6cf0beec1297d2e45e606162b19f3857efae4ff4
FW_1018_SHA=b7517a2b6670e964de9df23fec600e05e3c54e09bb62183b453a1ffaae5711bf

say(){ printf "\n\033[1;36m==> %s\033[0m\n" "$*"; }
die(){ printf "\033[1;31mError: %s\033[0m\n" "$*" >&2; exit 1; }

[ "$(uname)" = "Darwin" ] || die "macOS only."
command -v cargo >/dev/null || die "Rust required: https://rustup.rs"

say "Building the Rust filter and backend..."
cd "$REPO_DIR"
cargo build --release --quiet
cargo test --release --quiet >/dev/null || die "golden test failed"

say "Fetching the printer firmware (HP's, from the HPLIP plugin on OpenPrinting)..."
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
fetch_fw() {  # model sha
    local model=$1 sha=$2
    curl -fsSL "$PLUGIN_URL/hp_laserjet_$model.plugin" -o "$TMP/$model.plugin"
    mkdir -p "$TMP/$model"
    gunzip -c "$TMP/$model.plugin" | tar -xf - -C "$TMP/$model" "hp_laserjet_$model.fw.gz"
    gunzip -c "$TMP/$model/hp_laserjet_$model.fw.gz" > "$TMP/hp_laserjet_$model.fw"
    echo "$sha  $TMP/hp_laserjet_$model.fw" | shasum -a 256 -c --quiet - \
        || die "firmware checksum mismatch for $model"
}
fetch_fw 1020 "$FW_1020_SHA"
fetch_fw 1018 "$FW_1018_SHA"

say "Installing (needs sudo)..."
sudo mkdir -p "$FW_DIR"
sudo install -o root -g wheel -m 0644 "$TMP"/hp_laserjet_10{18,20}.fw "$FW_DIR/"
sudo install -o root -g wheel -m 0644 ppd/hp1020.ppd "$PPD_DIR/hp1020.ppd"
sudo install -o root -g wheel -m 0755 target/release/rastertozjs /usr/libexec/cups/filter/rastertozjs
# 0700 root-owned: CUPS runs the backend as root, which owns the USB interface.
sudo install -o root -g wheel -m 0700 target/release/hp1020 /usr/libexec/cups/backend/hp1020

say "Creating the printer queue..."
URI="$(target/release/hp1020 | awk '{print $2; exit}')"
if [ -z "$URI" ]; then
    URI="hp1020://HP/LaserJet%201020"
    echo "Printer not detected on USB right now; using $URI (any 1020 on USB)."
fi
lpadmin -p "$QUEUE" -E -v "$URI" -P "$PPD_DIR/hp1020.ppd" \
        -o printer-is-shared=false -D "HP LaserJet 1020" -L "USB (native ZJS)" 2>&1 \
    | grep -v "deprecated" || true
lpoptions -d "$QUEUE" >/dev/null

say "Done. Print to 'HP LaserJet 1020' from any app (Cmd-P)."
echo "Quick test:  lp -d $QUEUE /etc/hosts"
echo "Status:      target/release/hp1020 status"
echo "First job after power-on uploads firmware (~40 s extra). Logs: tail -f /var/log/cups/error_log"
