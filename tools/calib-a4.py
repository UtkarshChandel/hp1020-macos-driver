#!/usr/bin/env python3
"""Write an A4 calibration PDF: 10 mm frame with 10 mm ticks, centre cross, text 6-28 pt,
and an 11-step grey ramp. Used for print tests (see docs/TESTING.md).

usage: calib-a4.py [out.pdf]   (default calib-a4.pdf)
"""
import sys

mm = 72 / 25.4
W, H = 595.28, 841.89
ops = ["0.5 w", f"{10*mm:.2f} {10*mm:.2f} {W-20*mm:.2f} {H-20*mm:.2f} re S"]
for i in range(0, 200, 10):
    x = 10*mm + i*mm
    if x < W - 10*mm:
        ops.append(f"{x:.2f} {H-10*mm:.2f} m {x:.2f} {H-15*mm:.2f} l S")
ops.append(f"{W/2-20:.2f} {H/2:.2f} m {W/2+20:.2f} {H/2:.2f} l S "
           f"{W/2:.2f} {H/2-20:.2f} m {W/2:.2f} {H/2+20:.2f} l S")
for k in range(11):
    ops.append(f"{1 - k/10:.2f} g {30*mm + k*13*mm:.2f} {60*mm:.2f} {12*mm:.2f} {25*mm:.2f} re f")
ops.append("0 g")


def text(x, y, size, s):
    s = s.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")
    ops.append(f"BT /F1 {size} Tf {x:.2f} {y:.2f} Td ({s}) Tj ET")


text(25*mm, H-40*mm, 22, "HP LaserJet 1020 - native Rust driver")
text(25*mm, H-52*mm, 12, "A4 calibration: frame is 10 mm from every paper edge, ticks every 10 mm.")
text(25*mm, H-60*mm, 12, "Pipeline: cgpdftoraster -> rastertozjs (Rust JBIG/ZJS) -> hp1020 backend")
for i, sz in enumerate([6, 8, 10, 14, 18, 24]):
    text(25*mm, H-(90 + i*14)*mm, sz, f"{sz} pt  The quick brown fox jumps over the lazy dog")

content = "\n".join(ops).encode()
objs = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {W} {H}] /Contents 4 0 R "
    f"/Resources << /Font << /F1 5 0 R >> >> >>".encode(),
    b"<< /Length %d >>\nstream\n" % len(content) + content + b"\nendstream",
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
]
out = bytearray(b"%PDF-1.4\n")
offs = []
for i, o in enumerate(objs, 1):
    offs.append(len(out))
    out += b"%d 0 obj\n" % i + o + b"\nendobj\n"
xref = len(out)
out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
out += b"".join(b"%010d 00000 n \n" % o for o in offs)
out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, xref)
open(sys.argv[1] if len(sys.argv) > 1 else "calib-a4.pdf", "wb").write(out)
