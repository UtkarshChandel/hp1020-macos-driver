# Architecture

## Print pipeline

```
app ─Cmd-P─> CUPS ─> cgpdftoraster ──> rastertozjs ──────> hp1020 backend ──USB bulk──> printer
                    (Apple, built-in)   (Rust filter)        (Rust, nusb → IOKit)
                    PDF → CUPS raster   raster → ZjStream    firmware upload + job write
                    600 dpi, 1-bit K
```

CUPS picks the chain from the PPD line
`*cupsFilter2: "application/vnd.cups-raster application/vnd.hp-zjs 0 rastertozjs"`.
Apple's `cgpdftoraster` does all rendering and halftoning. We never touch PDF/PostScript.

## Components

| Path | Role |
|---|---|
| `src/jbig.rs` | JBIG1 (T.82) encoder, the subset the firmware uses. Byte-identical to jbig-kit 2.1 |
| `src/zjs.rs` | ZjStream writer: PJL header, `JZJZ` records, per-page JBIG, PJL trailer. Paper-code mapping |
| `src/raster.rs` | CUPS raster reader: RaSt/RaS2/RaS3, both byte orders, v2 compression, any depth → 1-bit (Bayer dither for >1 bpp) |
| `src/lib.rs` | Module wiring + `timestamp()` for the PJL job attribute |
| `src/bin/rastertozjs.rs` | CUPS filter. Reads options `PrintDensity` (1-5) and `EconoMode`, emits `PAGE:` lines |
| `src/bin/hp1020.rs` | CUPS backend `hp1020://` + manual tools (`status`, `load-firmware`, `send`) |
| `ppd/hp1020.ppd` | PPD: A4/Letter/Legal/Executive/A5/B5, 600 dpi 1-bit K raster, 4 mm margins (96 px), EconoMode, PrintDensity, 1284DeviceID |
| `packaging/build-pkg.sh` | Builds universal `.pkg` (see RELEASING.md) |
| `packaging/scripts/postinstall` | Runs as root: fetch firmware, create queue if printer connected, set default |
| `packaging/fetch-firmware.sh` | Downloads HPLIP plugins for 1020 + 1018, extracts `.fw`, checks SHA-256 |
| `packaging/uninstall.command` | Double-clickable uninstaller (removes queues with `hp1020:` URIs, files, pkg receipt) |
| `packaging/resources/*.html` | Installer welcome/conclusion pages (the license page is `LICENSE`) |
| `install.sh` / `uninstall.sh` | Source route: build the same `.pkg` and `installer -pkg` it / run uninstall.command |
| `tests/golden.rs` + `tests/golden/` | Byte-exact comparison with foo2zjs output |
| `tools/pbm2ras.py` | PBM → CUPS raster v3 (for feeding test bitmaps to the filter) |
| `tools/calib-a4.py` | A4 calibration PDF generator for print tests |

## Backend behaviour (`src/bin/hp1020.rs`)

* **Discovery** (no args): lists HP VID `0x03f0` devices whose PID is in `MODELS`
  (1020 `0x2b17`, 1018 `0x4117`) as
  `direct hp1020://HP/LaserJet%201020?serial=… "HP LaserJet 1020" … "MFG:…;MDL:…;" ""`.
  It sends **no USB requests** (a request could hit a booting printer); the Device ID string
  is synthesized.
* **Job** (6 or 7 args): `DEVICE_URI` `serial=` selects the unit. Waits (forever, like the
  stock `usb` backend) for the device, reporting `STATE: +offline-report`. Claims interface 0, then:
  1. Device ID (5 s timeout). If `FWVER:` is present, go to step 4.
  2. Upload `/Library/Printers/hp1020/firmware/<model>.fw` (or `$HP1020_FIRMWARE`, or next to
     the exe), drop the handle.
  3. Sleep `FW_BOOT` = 35 s, reopen, poll Device ID (10 s timeout, every 5 s, up to 60 s)
     until `FWVER:` appears. Otherwise exit `CUPS_BACKEND_STOP` with a power-cycle hint.
  4. Stream the job to bulk-OUT `0x01` (64 KB writer, 600 s write timeout because the printer
     legitimately blocks while warming up or out of paper).
* Logs go to stderr with `DEBUG:/INFO:/ERROR:` prefixes, which CUPS puts in `error_log`.

## Installed layout (from the .pkg)

```
/usr/libexec/cups/filter/rastertozjs                       0755 root:wheel
/usr/libexec/cups/backend/hp1020                           0700 root:wheel  (0700 → CUPS runs it as root)
/Library/Printers/PPDs/Contents/Resources/HP LaserJet 1020 Native.ppd   (Add Printer auto-match)
/Library/Printers/hp1020/firmware/hp_laserjet_10{18,20}.fw (downloaded by postinstall)
/Library/Printers/hp1020/{fetch-firmware.sh,uninstall.command,install.log}
CUPS queue: HP_LaserJet_1020   pkg id: io.github.utkarshchandel.hp1020-driver
```

## Why not …

* **Stock `usb://` backend + raw queue:** raw queues are refused on macOS
  ("lpadmin: Raw queues are no longer supported on macOS"), and something has to upload firmware.
* **foo2zjs + Ghostscript:** works but drags in gs and shell wrappers. Our filter needs neither,
  because cgpdftoraster already gives us 1-bit rasters.
* **Apple's HP 5.1.1 files (`rasterToHPZJS`):** x86_64-only binaries (Rosetta), closed source,
  installer blocked on macOS 13+. The prior-art repo anxkhn/hp1020-driver-mac does this.
* **libusb/rusb:** `nusb` is pure Rust over IOKit, so there is no dylib to ship.
