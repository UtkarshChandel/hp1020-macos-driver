# Releasing

Releases are built **locally** on a Mac. There is no CI: a GitHub Actions release
workflow was offered and deliberately skipped. Packages are **unsigned** (no Apple
Developer ID), so users see Gatekeeper's "Open Anyway" step, which the README documents.

## Steps

```bash
# 1. bump the version (it becomes the pkg version and file name)
$EDITOR Cargo.toml                         # version = "X.Y.Z"
cargo test --release
git commit -am "Release vX.Y.Z" && git push

# 2. build the installer (universal arm64 + x86_64, golden test included)
rustup target add aarch64-apple-darwin x86_64-apple-darwin   # once
packaging/build-pkg.sh                     # prints the sha256
(cd dist && shasum -a 256 HP-LaserJet-1020-Driver-X.Y.Z.pkg > HP-LaserJet-1020-Driver-X.Y.Z.pkg.sha256)

# 3. ideally test it on the real printer: user runs ./uninstall.sh, double-clicks the pkg, prints

# 4. publish (creates the tag too)
gh release create vX.Y.Z dist/HP-LaserJet-1020-Driver-X.Y.Z.pkg dist/HP-LaserJet-1020-Driver-X.Y.Z.pkg.sha256 \
   --title "vX.Y.Z: …" --notes "…"

# 5. verify the uploaded asset
gh release download vX.Y.Z -R UtkarshChandel/hp1020-macos-driver && shasum -a 256 -c *.sha256
```

Release notes should repeat the 3-step install and the "Open Anyway" instructions (see v1.0.0).

## What build-pkg.sh does (don't break it)

1. `cargo build --release` for both targets, `lipo` into universal binaries, `cargo test`.
2. Stages the payload root (paths and modes in ARCHITECTURE.md, "Installed layout").
3. `pkgbuild` → expand → **replace Payload/Bom with clean cpio/mkbom** (no `._` AppleDouble
   entries), remove `Scripts/._*`, fix `numberOfFiles` → `pkgutil --flatten`.
4. `productbuild` with a generated `distribution.xml`: welcome page, GPL license page,
   conclusion page, arm64 + x86_64, macOS ≥ 11, system domain only.

Check a built package:

```bash
pkgutil --expand dist/*.pkg /tmp/x && lsbom -p MUGsf /tmp/x/core.pkg/Bom   # no ._ entries, root wheel
pkgutil --payload-files dist/*.pkg | wc -l                                  # 18 for v1.0.0
```

## History

| Version | Date | Notes |
|---|---|---|
| v1.0.0 | 2026-10-19 | First release: double-click pkg, auto firmware + queue. Tested on 1020 Plus / macOS 15.6 arm64 |
