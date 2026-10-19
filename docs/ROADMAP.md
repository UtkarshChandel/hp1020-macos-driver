# Roadmap and open ideas

Nothing here is started. Ordered by value to users.

1. **Signed + notarized pkg.** Needs an Apple Developer ID ($99/yr; the maintainer has none as
   of v1.0.0). It would remove the Gatekeeper "Open Anyway" step. Use `productsign` with a
   "Developer ID Installer" cert, then `xcrun notarytool submit --wait` and `xcrun stapler staple`.
2. **GitHub Actions release.** On a `v*` tag, a macOS runner builds, tests and attaches the pkg,
   with signing switched on when secrets exist. Offered and skipped for v1.0.0.
3. **Bisect the firmware boot time** (needs a power-cycle per attempt). Can the 35 s wait be
   shorter, and is one Device-ID request with a long timeout safe during boot?
4. **Test the 1018** (`0x4117`, firmware already fetched) and the 1022 (has firmware in ROM.
   Probably only needs a `MODELS` entry with no firmware, untested).
5. **Homebrew cask** in a tap that runs the pkg (`brew install --cask …`).
6. **Check macOS 26.** The 1008a project needed root for USB there. Our backend already runs
   as root, but the no-sudo dev tools might not work.
7. **Back-channel status.** Read PJL USTATUS from bulk-IN to report paper-out / door-open to
   CUPS (`STATE: +media-empty-error`).
8. **PAPPL / IPP Everywhere printer application** (the long-term CUPS direction, since printer
   drivers are deprecated).
9. **ESP32-S3 Wi-Fi bridge** (side idea from the session). USB host on the ESP32-S3 plus
   firmware upload from flash. Clients do the rendering: either a browser page with pdf.js
   rasterizing (JBIG could run in WASM), or the ESP32 as an AirPrint/IPP server accepting
   PWG/URF raster and encoding JBIG band by band. A Raspberry Pi Zero 2 W with CUPS + foo2zjs is
   the easier alternative. This driver's encoder and the firmware-boot rule carry over.
