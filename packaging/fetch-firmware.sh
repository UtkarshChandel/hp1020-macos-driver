#!/bin/bash
# Download HP's LaserJet 1018/1020 firmware from the HPLIP plugin on OpenPrinting,
# verify it, and install it where the hp1020 backend looks for it.
# HP's license does not allow redistributing it, so it is fetched on each machine.
# Usage: sudo fetch-firmware.sh [dest-dir]
set -euo pipefail

DEST="${1:-/Library/Printers/hp1020/firmware}"
PLUGIN_URL=https://www.openprinting.org/download/printdriver/auxfiles/HP/plugins
# sha256 of the firmware inside HPLIP's per-model plugin (same bytes in hplip 2.8 .. 3.26)
SHA_1020=9a6d03c858d9cf64ba86fdbe6cf0beec1297d2e45e606162b19f3857efae4ff4
SHA_1018=b7517a2b6670e964de9df23fec600e05e3c54e09bb62183b453a1ffaae5711bf

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$DEST"

for model in 1020 1018; do
    sha_var="SHA_$model"
    curl -fsSL --retry 3 "$PLUGIN_URL/hp_laserjet_$model.plugin" -o "$TMP/$model.plugin"
    mkdir -p "$TMP/$model"
    gunzip -c "$TMP/$model.plugin" | tar -xf - -C "$TMP/$model" "hp_laserjet_$model.fw.gz"
    gunzip -c "$TMP/$model/hp_laserjet_$model.fw.gz" > "$TMP/hp_laserjet_$model.fw"
    echo "${!sha_var}  $TMP/hp_laserjet_$model.fw" | shasum -a 256 -c --quiet - \
        || { echo "firmware checksum mismatch for $model" >&2; exit 1; }
    install -m 0644 "$TMP/hp_laserjet_$model.fw" "$DEST/"
done
echo "Firmware installed in $DEST"
