#!/usr/bin/env python3
"""Wrap a P4 PBM as a 1-page CUPS raster v3 stream (1-bit K, 600 dpi), for golden tests.

usage: pbm2ras.py in.pbm out.ras [PageSizeName [crop]]
crop = pixels removed on every edge (foo2zjs -u/-l clipping), default 0
"""
import struct, sys

def read_pbm(path):
    d = open(path, 'rb').read()
    tok, i = [], 0
    while len(tok) < 3:
        while d[i:i+1].isspace(): i += 1
        if d[i:i+1] == b'#':
            while d[i:i+1] != b'\n': i += 1
            continue
        j = i
        while not d[j:j+1].isspace(): j += 1
        tok.append(d[i:j]); i = j
    assert tok[0] == b'P4'
    w, h = int(tok[1]), int(tok[2])
    return w, h, d[i+1:]

src, dst = sys.argv[1], sys.argv[2]
name = sys.argv[3] if len(sys.argv) > 3 else 'A4'
crop = int(sys.argv[4]) if len(sys.argv) > 4 else 0
w, h, bits = read_pbm(src)
bpl = (w + 7) // 8
ow, oh = w - 2 * crop, h - 2 * crop
obpl = (ow + 7) // 8
rows = []
for y in range(crop, h - crop):
    row = bits[y * bpl:(y + 1) * bpl]
    v = int.from_bytes(row, 'big') >> (bpl * 8 - crop - ow)  # drop right pad + right crop
    v &= (1 << ow) - 1
    rows.append((v << (obpl * 8 - ow)).to_bytes(obpl, 'big'))
hdr = bytearray(1796)
def put(off, val): struct.pack_into('<I', hdr, off, val)
put(276, 600); put(280, 600); put(340, 1)
sizes = {'A4': (595, 842), 'Letter': (612, 792)}
pw, ph = sizes.get(name, (595, 842))
put(352, pw); put(356, ph)
put(372, ow); put(376, oh); put(384, 1); put(388, 1); put(392, obpl); put(400, 3); put(420, 1)
hdr[1796-64:1796-64+len(name)] = name.encode()
open(dst, 'wb').write(b'3SaR' + bytes(hdr) + b''.join(rows))
