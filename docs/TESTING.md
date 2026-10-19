# Testing

## 1. Automated (always run)

```bash
cargo test --release
```

`tests/golden.rs` encodes `tests/golden/pattern.pbm` (1024×700: blank band, blocks,
random noise, gradient dither, repeated lines for TPB, a diagonal) and compares the full
stream byte-for-byte with `pattern.foo2zjs.zjs`.

That reference was produced with foo2zjs (jbig-kit 2.1), with the PJL timestamp pinned:

```bash
clang -O2 -o /tmp/foo2zjs foo2zjs.c jbig.c jbig_ar.c -w      # in a foo2zjs checkout
/tmp/foo2zjs -r600x600 -g1024x700 -p9 -m1 -n1 -d1 -s7 -z1 -P -L0 -T3 < pattern.pbm \
  | perl -pe 's/JobAttr4=\d{14}/JobAttr4=20000101000000/' > pattern.foo2zjs.zjs
```

Only regenerate it this way, from foo2zjs, never from our own output. When comparing with
foo2zjs on other images, use **byte-aligned widths**. foo2zjs leaks pad bits for others (see
BRING-UP-LOG §6).

## 2. No-sudo loop (filter + USB, without installing)

```bash
cupsfilter -p ppd/hp1020.ppd -o PageSize=A4 -m application/vnd.cups-raster doc.pdf > doc.ras
target/release/rastertozjs 1 user title 1 "PrintDensity=3" doc.ras > doc.zjs
target/release/hp1020 send doc.zjs        # uploads firmware first if the printer was power-cycled
```

* `cupsfilter` runs Apple's filters without cupsd. `-o page-ranges=1` limits pages.
* Decode check: `zjsdecode < doc.zjs` (record dump) and `zjsdecode -d out.pbm < doc.zjs`
  (decode the image with jbig-kit).
* `tools/pbm2ras.py in.pbm out.ras A4 96` turns any PBM into a raster, with optional
  foo2zjs-style edge crop.
* Test documents: `python3 tools/calib-a4.py calib.pdf` (A4 frame 10 mm from the edges,
  ticks, text sizes, grey ramp). Use **A4** documents on A4 paper. A Letter PDF clips at
  the right edge and looks like a bug.

## 3. Installed path (needs the user for sudo)

1. User runs `./install.sh` (or double-clicks `dist/*.pkg`), then checks
   `cat /Library/Printers/hp1020/install.log`.
2. `lpstat -v HP_LaserJet_1020` should show `hp1020://…serial=…`.
3. `lp -d HP_LaserJet_1020 calib.pdf` and wait until `lpstat -o` is empty. Then ask the user
   what printed (a photo helps).
4. **Firmware path:** ask the user to power-cycle. Confirm `hp1020 status` says
   "firmware: not loaded", then `lp` again. Expect ~37 s, "Waiting for the printer to start its
   firmware" in `lpstat -p`, then the page.

## Diagnostics

* `target/release/hp1020 status`: Device ID and firmware state. **Never run it in the
  ~35 s after a firmware upload** (it would wedge the printer).
* CUPS logs: the user runs `sudo cupsctl --debug-logging`, then `/var/log/cups/error_log`
  (lines prefixed `hp1020:` / `rastertozjs:`).
* Wedged printer (every USB request times out): only a power-cycle fixes it.
