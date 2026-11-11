"""Draw MessageCraft's icon (a blue tile with a white speech bubble, the mark the window uses) at
every size Windows asks for, and write assets/messagecraft.ico. Standard library only.

    python packaging/make_icon.py
"""

import struct
import zlib
from pathlib import Path

BLUE = (0x0F, 0x68, 0xF0)
SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]
SS = 4  # samples per pixel side, for smooth edges


def in_round_rect(x, y, x0, y0, x1, y1, r):
    cx = min(max(x, x0 + r), x1 - r)
    cy = min(max(y, y0 + r), y1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def sample(x, y):
    """Colour and coverage at a point of the 64-unit design grid."""
    if not in_round_rect(x, y, 2, 2, 62, 62, 14):
        return None
    bubble = in_round_rect(x, y, 13, 15, 51, 43, 9)
    tail = 41 <= y <= 52 and 18 <= x <= 30 and (x - 18) >= (y - 41) * 0.2 and (30 - x) >= (y - 41) * 1.05
    if bubble or tail:
        for dx in (23, 32, 41):
            if (x - dx) ** 2 + (y - 29) ** 2 <= 12:
                return BLUE
        return (255, 255, 255)
    return BLUE


def render(n):
    rows = bytearray()
    k = 64.0 / n
    for py in range(n):
        rows.append(0)
        for px in range(n):
            r = g = b = a = 0
            for sy in range(SS):
                for sx in range(SS):
                    c = sample((px + (sx + 0.5) / SS) * k, (py + (sy + 0.5) / SS) * k)
                    if c:
                        r += c[0]
                        g += c[1]
                        b += c[2]
                        a += 1
            if a:
                rows += bytes((r // a, g // a, b // a, 255 * a // (SS * SS)))
            else:
                rows += b"\0\0\0\0"
    return png(n, bytes(rows))


def png(n, raw):
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", n, n, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def main():
    images = [render(n) for n in SIZES]
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = b""
    for n, img in zip(SIZES, images):
        dim = 0 if n >= 256 else n
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(img), offset)
        offset += len(img)
    out = Path(__file__).resolve().parent.parent / "assets" / "messagecraft.ico"
    out.write_bytes(header + entries + b"".join(images))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
