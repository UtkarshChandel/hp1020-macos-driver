#!/bin/bash
# Build HP-LaserJet-1020-Driver-<version>.pkg: universal (arm64 + x86_64) binaries,
# firmware fetched at install time, queue created by postinstall.
# Usage: packaging/build-pkg.sh   -> dist/HP-LaserJet-1020-Driver-<version>.pkg
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
VERSION="$(awk -F'"' '/^version/{print $2; exit}' Cargo.toml)"
ID=io.github.utkarshchandel.hp1020-driver
OUT=dist
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

for t in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --release --quiet --target "$t"
done
cargo test --release --quiet >/dev/null

ROOT="$WORK/root"
mkdir -p "$ROOT/usr/libexec/cups/filter" "$ROOT/usr/libexec/cups/backend" \
         "$ROOT/Library/Printers/PPDs/Contents/Resources" "$ROOT/Library/Printers/hp1020/firmware"
for bin in rastertozjs hp1020; do
    lipo -create -output "$WORK/$bin" \
        "target/aarch64-apple-darwin/release/$bin" "target/x86_64-apple-darwin/release/$bin"
done
install -m 0755 "$WORK/rastertozjs" "$ROOT/usr/libexec/cups/filter/rastertozjs"
# 0700 root-owned: CUPS runs the backend as root, which owns the USB interface.
install -m 0700 "$WORK/hp1020" "$ROOT/usr/libexec/cups/backend/hp1020"
install -m 0644 ppd/hp1020.ppd "$ROOT/Library/Printers/PPDs/Contents/Resources/HP LaserJet 1020 Native.ppd"
install -m 0755 packaging/fetch-firmware.sh packaging/uninstall.command "$ROOT/Library/Printers/hp1020/"

pkgbuild --quiet --root "$ROOT" --ownership recommended --identifier "$ID" --version "$VERSION" \
    --scripts packaging/scripts --install-location / "$WORK/pkgbuild.pkg"

# pkgbuild stores extended attributes (e.g. com.apple.provenance, which cannot always be
# stripped) as AppleDouble "._" entries; a "._hp1020" would even show up as a CUPS
# backend. Rebuild the payload and BOM with plain cpio, then flatten again.
pkgutil --expand "$WORK/pkgbuild.pkg" "$WORK/core"
( cd "$ROOT" && find . | LC_ALL=C sort | cpio -o --format odc -R 0:0 2>/dev/null ) | gzip -9 -c > "$WORK/core/Payload"
mkbom "$ROOT" "$WORK/raw.bom"
lsbom "$WORK/raw.bom" | awk -F'\t' 'BEGIN{OFS="\t"} {$3="0/0"; print}' > "$WORK/filelist"
rm -f "$WORK/core/Bom" && mkbom -i "$WORK/filelist" "$WORK/core/Bom"
rm -f "$WORK"/core/Scripts/._*
sed -i '' "s/numberOfFiles=\"[0-9]*\"/numberOfFiles=\"$(wc -l < "$WORK/filelist" | tr -d ' ')\"/" "$WORK/core/PackageInfo"
pkgutil --flatten "$WORK/core" "$WORK/core.pkg"
if lsbom -s "$WORK/core/Bom" | grep -q '/\._'; then echo "AppleDouble entries left in BOM" >&2; exit 1; fi

mkdir -p "$WORK/res"
cp packaging/resources/*.html "$WORK/res/"
cp LICENSE "$WORK/res/LICENSE.txt"
cat > "$WORK/distribution.xml" <<XML
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>HP LaserJet 1020 Driver $VERSION</title>
    <welcome file="welcome.html"/>
    <license file="LICENSE.txt"/>
    <conclusion file="conclusion.html"/>
    <options customize="never" require-scripts="false" hostArchitectures="arm64,x86_64"/>
    <domains enable_anywhere="false" enable_currentUserHome="false" enable_localSystem="true"/>
    <allowed-os-versions><os-version min="11.0"/></allowed-os-versions>
    <choices-outline><line choice="default"/></choices-outline>
    <choice id="default" title="HP LaserJet 1020 Driver"><pkg-ref id="$ID"/></choice>
    <pkg-ref id="$ID" version="$VERSION" onConclusion="none">core.pkg</pkg-ref>
</installer-gui-script>
XML

mkdir -p "$OUT"
PKG="$OUT/HP-LaserJet-1020-Driver-$VERSION.pkg"
COPYFILE_DISABLE=1 productbuild --quiet --distribution "$WORK/distribution.xml" --resources "$WORK/res" \
    --package-path "$WORK" "$PKG"
shasum -a 256 "$PKG"
