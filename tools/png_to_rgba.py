#!/usr/bin/env python3
"""Convert a PNG into the raw RGBA bytes embedded by the application.

egui's IconData wants linear, row-major RGBA8 data without any header, so the
window icon can be embedded with include_bytes! and no image decoder crate.

Usage:
    ./tools/png_to_rgba.py assets/icon.png assets/icon.rgba
"""

import struct
import sys
import zlib


def read_png(path):
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path}: not a PNG file")

    pos = 8
    width = height = depth = color = interlace = None
    idat = bytearray()
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos : pos + 8])
        body = data[pos + 8 : pos + 8 + length]
        if kind == b"IHDR":
            width, height, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
        pos += 12 + length

    if depth != 8 or color != 6 or interlace != 0:
        raise SystemExit(
            f"{path}: expected non-interlaced 8-bit RGBA, "
            f"got depth={depth} color={color} interlace={interlace}"
        )
    return width, height, zlib.decompress(bytes(idat))


def unfilter(width, height, raw):
    stride = width * 4
    out = bytearray(stride * height)
    prev = bytearray(stride)
    pos = 0
    for y in range(height):
        kind = raw[pos]
        line = bytearray(raw[pos + 1 : pos + 1 + stride])
        pos += 1 + stride
        for i in range(stride):
            a = line[i - 4] if i >= 4 else 0
            b = prev[i]
            c = prev[i - 4] if i >= 4 else 0
            v = line[i]
            if kind == 0:
                pass
            elif kind == 1:
                v = (v + a) & 0xFF
            elif kind == 2:
                v = (v + b) & 0xFF
            elif kind == 3:
                v = (v + (a + b) // 2) & 0xFF
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                v = (v + pred) & 0xFF
            else:
                raise SystemExit(f"unknown PNG filter {kind} on row {y}")
            line[i] = v
        out[y * stride : (y + 1) * stride] = line
        prev = line
    return out


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    src, dst = sys.argv[1], sys.argv[2]
    width, height, raw = read_png(src)
    rgba = unfilter(width, height, raw)
    # Fully transparent pixels often carry noise; normalise them so the
    # embedded buffer compresses predictably.
    for i in range(0, len(rgba), 4):
        if rgba[i + 3] == 0:
            rgba[i : i + 4] = b"\x00\x00\x00\x00"
    open(dst, "wb").write(rgba)
    print(f"{dst}: {width}x{height}, {len(rgba)} bytes")


if __name__ == "__main__":
    main()
