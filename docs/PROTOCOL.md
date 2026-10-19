# Protocol reference (LaserJet 1018 / 1020 family)

Sources: foo2zjs `foo2zjs.c` / `zjs.h` (model 1 = `-z1`), jbig-kit 2.1, our own captures.
Everything here was confirmed by printing.

## Job stream

```
ESC%-12345X@PJL JOB\n
@PJL SET JAMRECOVERY=OFF\n
@PJL SET DENSITY=<1-5>\n
@PJL SET ECONOMODE=<ON|OFF>\n
@PJL SET RET=MEDIUM\n
@PJL INFO STATUS\n
@PJL USTATUS DEVICE = ON\n
@PJL USTATUS JOB = ON\n
@PJL USTATUS PAGE = ON\n
@PJL USTATUS TIMED = 30\n
@PJL SET JOBATTR="JobAttr4=YYYYmmddHHMMSS"  NUL  ESC%-12345X
JZJZ                                   (magic; all record fields big-endian)
  START_DOC   reserved 0x24, 3 items: DMCOLLATE=0, DMDUPLEX=1, PAGECOUNT=0
  per page:
    START_PAGE reserved 0x9c, 13 items (order matters, as below)
    JBIG_BIH   20-byte payload
    JBIG_BID   ≤ 65536-byte payload each; the last is zero-padded
    END_JBIG
    END_PAGE
  END_DOC
ESC%-12345X@PJL EOJ\nESC%-12345X
```

## Records

Header, 16 bytes: `size u32` (includes header), `type u32`, `items u32`, `reserved u16`,
`signature "ZZ"`. Item, 12 bytes: `size u32 = 12`, `item u16`, `type u8 = 1 (uint32)`,
`param u8 = 0`, `value u32`.

| type | record | | item | id |
|---|---|---|---|---|
| 0 | START_DOC | | PAGECOUNT | 0 |
| 1 | END_DOC | | DMCOLLATE | 1 |
| 2 | START_PAGE | | DMDUPLEX | 2 |
| 3 | END_PAGE | | DMPAPER | 3 |
| 4 | JBIG_BIH | | DMCOPIES | 4 |
| 5 | JBIG_BID | | DMDEFAULTSOURCE | 5 (7 = auto) |
| 6 | END_JBIG | | DMMEDIATYPE | 6 (1 = plain) |
| | | | NBIE | 7 (1 = mono) |
| | | | RESOLUTION_X/Y | 8 / 9 |
| | | | RASTER_X/Y | 12 / 13 |
| | | | VIDEO_BPP / X / Y | 16 / 17 / 18 |
| | | | ECONOMODE | 23 |

START_PAGE item order (as foo2zjs writes it): ECONOMODE, VIDEO_X, VIDEO_Y, VIDEO_BPP=1,
RASTER_X, RASTER_Y, NBIE=1, RESOLUTION_X=600, RESOLUTION_Y=600, DMDEFAULTSOURCE=7,
DMCOPIES, DMPAPER, DMMEDIATYPE=1.

* `VIDEO_X` = `RASTER_X` = **real** raster width. The JBIG image width (in the BIH) is padded
  up to a multiple of **128** px.
* **No START_PLANE/END_PLANE** for the 1020 (foo2zjs `-P`: plane number 0 → omitted).
* Last BID record padding: `16 + roundup4(len) − len` zero bytes (foo2zjs `ExtraPad` = 16).
* BID payload blocks are split at exactly 65536 bytes. foo2zjs's comment: "Printer does strange
  things otherwise."
* Paper codes (Windows DEVMODE): Letter 1, Legal 5, Executive 7, A4 9, A5 11, B5 13.
* Copies: we send DMCOPIES = 1 and set `*cupsManualCopies: True`, so CUPS/cgpdftoraster
  produce the copies.

## JBIG parameters (BIH bytes)

```
00 00 01 00 | XD (BE32, padded width) | YD (BE32) | L0 = 00 00 00 80 | MX=00 MY=00 | ORDER=03 | OPTIONS=5c
```

* Single plane, D = 0 (one resolution layer), L0 = 128 lines per stripe.
* ORDER 0x03 = ILEAVE | SMID. OPTIONS 0x5c = LRLTWO | TPDON | TPBON | DPON (TPDON/DPON are
  inert with one layer; they are kept because foo2zjs emits them).
* **MX = 0**: foo2zjs forces `JbgOptions[3] = 0` for the 1020 family, so there are no
  adaptive-template moves. This is why our encoder can be small.
* Two-line template context: `((line_h2 >> 10) & 0x3f0) | ((line_h1 >> 9) & 0x00f)`.
  TPB context `0x195`.
* Stripes end with `FF 02` (SDNORM). The arithmetic-coder probability state and the
  reference lines **carry across stripes** (no SDRST). Per stripe: flush, strip trailing
  `00` bytes, and if the last byte is then `FF` re-append `00` (jbig-kit
  `jbg_buf_remove_zeros`).
* Input polarity: 1 = black, MSB first. Bits past the real width must be 0 (see
  BRING-UP-LOG, "foo2zjs leaks pad bits").

## Raster input (what cgpdftoraster gives us)

With our PPD (`cupsColorSpace 3` = K, `cupsBitsPerColor 1`, `HWResolution 600`), Apple's
`cgpdftoraster` emits **RaS3 (uncompressed, little-endian sync `3SaR`), 1 bpp, 1 = black**,
already cropped to the PPD ImageableArea:

| page | raster |
|---|---|
| Letter | 4908 × 6408 |
| A4 | 4766 × 6825 |

foo2zjs gets the same effect from Ghostscript with clipping `-u96x96 -l96x96` at 600×600 dpi.
That's 96 px = 4 mm per edge; the wrapper's x value 192 is halved for 600 dpi.

## USB

* VID `0x03f0`, PID `0x2b17` (1020 and 1020 Plus), `0x4117` (1018).
* Interface 0, alt 0, class 7/1/2 (printer, bidirectional). Bulk OUT `0x01`, bulk IN `0x81`,
  max packet 512. No IPP-USB alt setting.
* IEEE-1284 Device ID: class request `GET_DEVICE_ID` (bmRequestType class|interface,
  bRequest 0, wValue 0, wIndex 0). The first 2 bytes are the big-endian length.

## Firmware blob

From HPLIP's plugin (`hp_laserjet_1020.plugin` = gzip tar → `hp_laserjet_1020.fw.gz`):

```
ESC%-12345X@PJL ENTER LANGUAGE=ACL\r\n     33 bytes
00 AC C0 DE                                 magic
00 01 F7 AC                                 BE32 = 128940 (size after the version string)
"20080822"                                  8-byte version (not counted in the size)
7F 45 4C 46 …                               ARM ELF image … checksum
ESC%-12345X                                 UEL
```

Total 128999 bytes, SHA-256 `9a6d03c8…fae4ff4`. The printer then reports `FWVER:20080222`.
Send it raw to bulk-OUT as-is. The 1018 blob has the same layout (129287 bytes,
SHA-256 `b7517a2b…a5711bf`). Both SHAs are pinned in `packaging/fetch-firmware.sh`.

Apple's `hp1020.acl` (HP Printer Drivers 5.1.1) is an **older** image (version `20050309`,
128380 bytes, raw body without the PJL/magic header). Apple's `rasterToHPZJS` adds the
header/body/trailer itself. It was not used.
