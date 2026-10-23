#!/usr/bin/env python3
"""Convert raw <name>.<W>x<H>.rgba screenshots (from MERIDIAN_SCRIPT runs) to PNG."""
import re, struct, sys, zlib, os

def png(path):
    m = re.search(r"\.(\d+)x(\d+)\.rgba$", path)
    w, h = int(m.group(1)), int(m.group(2))
    data = open(path, "rb").read()
    rows = b"".join(b"\x00" + data[y * w * 4:(y + 1) * w * 4] for y in range(h))
    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    out = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    out += chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b"")
    dst = re.sub(r"\.\d+x\d+\.rgba$", ".png", path)
    open(dst, "wb").write(out)
    os.remove(path)
    print(dst)

for p in sys.argv[1:]:
    png(p)
