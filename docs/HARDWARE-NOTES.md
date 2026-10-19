# Hardware notes (measured on the maintainer's unit)

Unit: **HP LaserJet 1020 Plus**, serial `S416GT1`, connected through a Belkin USB-C hub to a
MacBook Air (Apple Silicon), macOS 15.6 (24G84). Measured 2026-10-19.

## Identity

* Enumerates as a plain **"HP LaserJet 1020"**, `03f0:2b17`. The "Plus" is not visible over
  USB, and it uses the 1020 firmware.
* Device ID before firmware:
  `MFG:Hewlett-Packard;MDL:HP LaserJet 1020;CMD:ACL;CLS:PRINTER;DES:HP LaserJet 1020;`
* Device ID with firmware running: the same plus `FWVER:20080222;`.
* One interface (7/1/2), bulk OUT 0x01 / IN 0x81. No IPP-over-USB alternate setting, so
  macOS's `ippusbd` does **not** grab it (unlike the 1008a, where that broke printing).

## Firmware boot timing (the important one)

The printer forgets its firmware at every power-off. After the 129 KB upload:

* The bulk write completes normally. The printer makes no visible or audible reaction.
* **About 20-40 s of boot.** During that window:
  * Device-ID control requests time out ("transfer was cancelled"), and **after that the
    printer stays dead** (Device ID and bulk writes all time out) until it is power-cycled.
    Seen with polling every 2-5 s starting at t = 2-4 s, three times.
  * A job sent at t = 20 s (with no earlier polling) was **accepted by USB but silently
    dropped**. No page.
* At t ≈ 40 s, Device ID answered with `FWVER`, and the next job printed.
* Production behaviour: sleep 35 s after upload, then poll. Verified through CUPS after a
  power-cycle: the whole job took ~37 s, then the page printed.
* Not yet bisected: the exact boot time, and whether one request with a long timeout
  (instead of short polls) would be safe. Each experiment costs a manual power-cycle.

## macOS behaviour

* **User-level USB works** on macOS 15.6 via `nusb` (IOKit): enumerate, claim, control and
  bulk transfers, no root. (The 1008a project found root necessary on macOS 26. Unverified
  here.)
* The CUPS backend is installed 0700 root, so CUPS runs it as root. The backend sandbox
  allowed USB access without issues.
* Stock CUPS `usb://` backend lists the printer (`usb://Hewlett-Packard/HP%20LaserJet%201020?serial=S416GT1`),
  but raw queues are refused, and it can't upload firmware.

## Print quality observations

* Text, 1-bit dithered photos and grey ramps print correctly. The grey ramp's
  coarse-looking halftone is cgpdftoraster's dither, not the driver.
* A slight **tilt** on prints is paper feed skew (tray guides), not data.
* Position: an A4 calibration frame drawn 10 mm from every edge came out evenly placed.
* "First page after power-on is slow" (~40 s) is the firmware upload + boot. Every page after
  that is immediate.
