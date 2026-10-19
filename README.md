# HP LaserJet 1020 / 1020 Plus / 1018 on macOS (native Rust driver)

`Cmd-P` printing on Apple Silicon macOS for HP's host-based ZjStream lasers. HP ships no
driver for modern macOS. Apple's old HP package has an x86-only filter and an installer
that refuses macOS 13+. This project replaces everything with two small Rust binaries:
no Ghostscript, no foo2zjs, no Rosetta, no libusb, no Homebrew runtime dependencies.

Tested on macOS 15.6 (arm64) with an HP LaserJet 1020 Plus (USB `03f0:2b17`).

## Install

1. Download **`HP-LaserJet-1020-Driver-<version>.pkg`** from the
   [latest release](https://github.com/UtkarshChandel/hp1020-macos-driver/releases/latest).
2. Plug the printer in over USB and switch it on.
3. Double-click the `.pkg` and click through the installer.

That's it. The installer installs the driver, downloads the printer firmware, and adds
**HP LaserJet 1020** as your default printer. Print from any app with `Cmd-P`. If the
printer wasn't connected during install, plug it in later and add it in
**System Settings → Printers & Scanners → Add Printer**; the driver is picked automatically.

**"Apple could not verify…" warning:** the package is not signed with a paid Apple
Developer ID, so the first time you open it macOS blocks it. Click **Done**, open
**System Settings → Privacy & Security**, scroll down, click **Open Anyway** next to the
package name, and confirm. (Or in Terminal: `xattr -d com.apple.quarantine ~/Downloads/HP-LaserJet-1020-Driver-*.pkg`.)

**Uninstall:** double-click `/Library/Printers/hp1020/uninstall.command`.

**Build from source instead:** `./install.sh` (needs [Rust](https://rustup.rs)). It builds the
same package locally and installs it. `packaging/build-pkg.sh` only builds `dist/*.pkg`.

## How it works

```
any app ─Cmd-P─> CUPS ─> cgpdftoraster ─(1-bit CUPS raster)─> rastertozjs ─(ZjStream)─> hp1020 backend ─USB─> printer
                         (Apple's)                           (Rust filter)              (Rust, nusb/IOKit)
```

* **`rastertozjs`** (`src/bin/rastertozjs.rs`): reads CUPS raster and writes ZjStream:
  PJL header, `JZJZ` records, one JBIG1 image per page. The PPD asks `cgpdftoraster` for
  600 dpi 1-bit black, cropped to the 4 mm hardware margins, so the filter only has to
  pad each page to a multiple of 128 px and encode it.
* **`src/jbig.rs`**: a ~300-line JBIG1 (T.82) encoder covering exactly what the firmware
  uses: one layer, two-line template, typical prediction, 128-line stripes, no AT moves.
  Its output is **byte-identical to jbig-kit/foo2zjs** (`tests/golden.rs`, plus a 2.2 MB
  stress page checked during bring-up).
* **`hp1020`** (`src/bin/hp1020.rs`): a CUPS backend (`hp1020://`). It finds the printer by
  vendor/product ID, claims interface 0 (class 7/1/2, bulk-OUT `0x01`), and streams the job.

### Firmware

These printers have **no firmware in flash**. After every power-on, the host must upload a
~129 KB blob (`@PJL ENTER LANGUAGE=ACL` + an ARM ELF) before the printer can print. The
backend reads the IEEE-1284 Device ID. If there is no `FWVER:` field, it uploads
`/Library/Printers/hp1020/firmware/hp_laserjet_1020.fw` and then waits for the printer to
boot. The installer downloads HP's firmware from the HPLIP plugin on OpenPrinting and
checks its SHA-256 (`/Library/Printers/hp1020/fetch-firmware.sh`; rerun it with sudo if
you installed offline). HP's license forbids redistributing it, so it is in neither this
repository nor the package.

**Gotcha found during bring-up:** the firmware takes ~30 s to boot. Any USB request sent
during that window (even a Device-ID control request) is lost or wedges the printer until
it is power-cycled. So the backend sleeps 35 s after an upload before it touches the device
again. Only the first job after power-on pays this cost.

## Tools

```bash
target/release/hp1020 status               # model, serial, Device ID, firmware state
target/release/hp1020 load-firmware        # upload firmware now (if not loaded)
target/release/hp1020 send job.zjs         # raw send (loads firmware first if needed)
target/release/rastertozjs 1 u t 1 "" page.ras > page.zjs
python3 tools/pbm2ras.py in.pbm out.ras A4 96   # PBM -> CUPS raster, for golden tests
```

User-level USB access works on macOS 15, so all of the above runs without sudo. The CUPS
backend is installed root-owned `0700`, so CUPS runs it as root.

## Options

`Toner Saver` (EconoMode) and `Print Density` (1-5) appear in the print dialog. Paper sizes:
A4, Letter, Legal, Executive, A5, B5.

## License

GPL-2.0-or-later (see `LICENSE`). `src/jbig.rs` is a port of jbig-kit's encoder, and the
ZjStream layout follows foo2zjs; both are GPL. HP's firmware is not part of this repository.

## Credits

Protocol knowledge comes from Rick Richardson's [foo2zjs](https://github.com/OpenPrinting/foo2zjs)
and Markus Kuhn's jbig-kit. The firmware is HP's (from HPLIP).
