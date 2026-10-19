# Bring-up log: what was tried, what failed, why

Chronological record of the 2026-10-19 bring-up session (one day, from nothing to v1.0.0).
Read this before trying "obvious" things; several of them were already tried.

## 0. Starting point and method

The method came from the sibling 1008a project (`~/Desktop/hp-laser-1008a-macos`):
use a known-good encoder as an oracle, diff byte streams, keep the native part tiny, and
handle macOS USB quirks in a root CUPS backend.

Prior art found:
* **foo2zjs** (OpenPrinting mirror, github.com/OpenPrinting/foo2zjs): open-source ZjStream
  encoder, model `-z1` for the 1018/1020/1022. This became the protocol reference and oracle.
* **anxkhn/hp1020-driver-mac**: extracts `rasterToHPZJS`, `commandToHPZJS`, `hp1020.acl` and
  the LaserJet 1022 PPD from Apple's HP Printer Drivers 5.1.1. The README says "universal",
  but `file` shows `rasterToHPZJS` is **x86_64 only**.
* **faradayfury/printer-all**: ARM64 foo2zjs builds, 1020 marked untested.
* HP's statement: no macOS driver for 1020 on macOS 13+.

## 1. Identify the device: ✅

`ioreg -p IOUSB -l -w0`: `03f0:2b17` "HP LaserJet 1020", serial S416GT1. A small `nusb` probe
as a normal user listed the interface (7/1/2, EP 0x01/0x81) and read the Device ID. There
was no `FWVER`, so the firmware was not loaded. **Discovery: no sudo needed for USB on macOS 15.**

## 2. Get the firmware: ✅

* `openprinting.org/download/printdriver/auxfiles/HP/plugins/hp_laserjet_1020.plugin`
  (117 KB, a gzip tar from HPLIP 2.8.5) → `hp_laserjet_1020.fw.gz` → a 128999-byte ready-to-send blob.
* The newest `hplip-3.26.6-plugin.run` (makeself archive, 11 MB) contains a **byte-identical**
  `hp_laserjet_1020.fw`. The firmware hasn't changed in 18 years.
* Extracting makeself without running it: `offset=$(head -n 591 plugin.run | wc -c)`, then
  `tail -c +$((offset+1)) | gzip -cd | tar -x` (591 = the line count in its header).
* HP's plugin LICENSE: "You may not copy the Software onto any public network". So
  download at install time, never commit.

## 3. First firmware upload: ❌ printer wedged (3 times)

| Try | What | Result |
|---|---|---|
| a | `write_all` 129 KB to EP 0x01, then Device ID every 2 s | all requests time out; bulk writes time out too; nothing visible on the printer |
| b | power-cycle; upload in 4 KB `transfer_blocking` chunks; poll every 3 s | same |
| c | power-cycle; upload while a thread keeps reading bulk-IN (theory: like Linux `usblp`, the printer wants its status read); poll from t = 4 s | same. **Bulk-IN theory disproved** |
| d | ruled out firmware mismatch: the 2008 and 2026 HPLIP blobs are identical, and Apple's is older | |
| e | power-cycle; upload, **no polling**, send a test page at t = 20 s | USB accepted the page, but nothing printed |
| f | Device ID at t ≈ 40 s | `FWVER:20080222`. **Firmware was running.** |
| g | re-send the same page | **printed perfectly** |

Conclusion: the firmware boots for roughly 20-40 s. Requests during boot are dropped, or they
wedge the printer until power-cycle. Fix: wait 35 s after upload before any request (see
HARDWARE-NOTES.md).

## 4. Oracle stream: ✅

* Built foo2zjs natively: `clang -O2 -o /tmp/foo2zjs foo2zjs.c jbig.c jbig_ar.c -w` (and
  `zjsdecode`). No Ghostscript: fed it a Python-generated PBM directly.
* Command for an A4 page at 600 dpi:
  `foo2zjs -r600x600 -g4960x7016 -p9 -m1 -n1 -d1 -s7 -z1 -P -L0 -u96x96 -l96x96 -T3 < page.pbm`
  (flags from foo2zjs-wrapper: `-z1` model 1020, `-P` no START_PLANE, `-L0` no logical clip).
* That stream (frame, diagonal, bars) printed correctly. This proved the transport.

## 5. What cgpdftoraster produces: ✅

`cupsfilter -p ppd/hp1020.ppd -m application/vnd.cups-raster in.pdf > out.ras` runs Apple's
filter chain **without the CUPS daemon or sudo**. The output is RaS3 little-endian, 1-bit K,
already cropped to the ImageableArea (Letter 4908×6408 = exactly foo2zjs's clipped size).
So the filter needs no cropping or dithering, only padding and encoding.

## 6. Rust encoder: ✅ byte-identical on the first try

* Ported jbig-kit's QM coder (`jbig_ar.c`) and the lowest-layer two-line-template path of
  `encode_sde` (`jbig.c`), with MX = 0.
* Compared with foo2zjs, ignoring the PJL timestamp: bars page ✅, cgpdftoraster text page ✅.
* **Apparent mismatch on a noise page:** this was input semantics, not an encoder bug.
  **foo2zjs leaks pad bits**: when the clipped width isn't a multiple of 8 (5100 − 192 = 4908),
  foo2zjs copies whole bytes, so 4 source pixels past the right edge end up in the last
  byte. Our filter masks them to 0, which is correct. With a byte-aligned width
  (5104 − 192 = 4912) the 2.2 MB stress page (35 BID blocks, carries, TPB runs) is identical.
* Round trip: `zjsdecode -d out.pbm < job.zjs` decodes our stream back to the original raster.

## 7. Printed via Rust, then via CUPS: ✅

* README text page and a dithered photo printed through `cupsfilter` → `rastertozjs` →
  `hp1020 send`.
* After `install.sh` (the user ran the sudo part): `lp` printed. Artifacts seen: tilt =
  paper skew; right-edge clipping = the test PDF was **US Letter** printed on A4 (not a bug).
* Power-cycle → `lp` A4 calibration page → the backend uploaded firmware, waited, and printed
  (~37 s total). The calibration frame was evenly placed.

## 8. Packaging: ✅ after one trap

* `pkgbuild` turned the un-removable `com.apple.provenance` xattr (`xattr -cr` has no effect
  inside the agent's sandbox) into **AppleDouble `._` entries** in the BOM and payload,
  including `usr/libexec/cups/backend/._hp1020`. `COPYFILE_DISABLE=1` did not help.
* Fix in `build-pkg.sh`: run pkgbuild (for PackageInfo/Scripts), expand it, replace
  `Payload` with `find | cpio -o --format odc -R 0:0 | gzip`, rebuild `Bom` with
  `mkbom -i` (uid/gid forced to 0/0), delete `Scripts/._*`, fix `numberOfFiles`, flatten,
  then `productbuild`. The script fails if any `._` entry is left.
* Fresh install test: `./uninstall.sh` → double-click `.pkg` → firmware fetched, queue
  auto-created as default, page printed.
* Released as v1.0.0 (unsigned; no Apple Developer ID).

## Dead ends, summarised

* Polling the printer right after the firmware upload: wedges it.
* Draining bulk-IN during the upload: irrelevant.
* Suspecting the firmware version: the HPLIP blob is the right one.
* `lpadmin -m raw`: refused on macOS.
* Stripping xattrs to fix pkgbuild: impossible in the sandbox; rebuild the payload instead.
* Using `/tmp` for anything the CUPS backend writes: the backend sandbox blocks it (known
  from 1008a). Log to stderr.
