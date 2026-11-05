#!/usr/bin/env python3
"""Generate the OxyStudio application icon (PNG sizes + Windows .ico).

Standard library only (zlib + struct), no third-party dependencies.
Usage: python3 packaging/icons/make_icons.py
"""
import math
import struct
import zlib
from pathlib import Path

OUT = Path(__file__).resolve().parent
SIZES = [16, 24, 32, 48, 64, 128, 256, 512, 1024]
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]

BG = (0x16, 0x18, 0x22)
CYAN_ACCENT = (0x38, 0xBD, 0xF8)
CYAN_GLOW = (0x02, 0x84, 0xC7)
WHITE = (0xF0, 0xF9, 0xFF)


def srgb(x: float) -> float:
    x = min(max(x, 0.0), 1.0)
    return x * 12.92 if x <= 0.0031308 else 1.055 * x ** (1 / 2.4) - 0.055


def shade(u: float, v: float, size: int):
    # Rounded-square background
    x, y = u * 2 - 1, v * 2 - 1
    r = 0.36
    qx, qy = max(abs(x) - (1 - r), 0.0), max(abs(y) - (1 - r), 0.0)
    d_box = math.hypot(qx, qy) - r
    if d_box > 0:
        return (0.0, 0.0, 0.0, 0.0)

    # Base background color with subtle radial gradient
    dist_center = math.hypot(x, y)
    bg_factor = 1.0 - 0.25 * dist_center
    col = [c * bg_factor for c in BG]

    # Astroid / 4-pointed star emblem: |x|^(2/3) + |y|^(2/3) <= a^(2/3)
    ax, ay = abs(x), abs(y)
    p = 0.75
    star_val = (ax ** p + ay ** p) ** (1.0 / p)
    star_radius = 0.65

    # Center glow
    glow = max(0.0, 1.0 - dist_center / 0.8) ** 2 * 0.4
    col = [c + g * glow * 255.0 for c, g in zip(col, (0.1, 0.3, 0.6))]

    # Inside the star
    if star_val < star_radius:
        t = star_val / star_radius
        # Shading from center white to cyan edges
        core = max(0.0, 1.0 - dist_center / 0.35) ** 1.5
        col = [
            WHITE[i] * core + CYAN_ACCENT[i] * (1.0 - core) * (1.0 - 0.3 * t)
            for i in range(3)
        ]

    # Star outline glow
    border_dist = abs(star_val - star_radius)
    border_width = max(0.04, 2.0 / size)
    if border_dist < border_width:
        t = 1.0 - border_dist / border_width
        col = [c * (1.0 - t) + CYAN_ACCENT[i] * t for i, c in enumerate(col)]

    # Orbit ring
    ring_radius = 0.58
    ring_dist = abs(dist_center - ring_radius)
    ring_width = max(0.02, 1.2 / size)
    if ring_dist < ring_width:
        t = (1.0 - ring_dist / ring_width) * 0.45
        col = [c * (1.0 - t) + CYAN_ACCENT[i] * t for i, c in enumerate(col)]

    return (min(255.0, col[0]), min(255.0, col[1]), min(255.0, col[2]), 255.0)


def render(size: int, ss: int = 4) -> bytes:
    rows = bytearray()
    for j in range(size):
        rows.append(0)  # PNG filter: none
        for i in range(size):
            acc = [0.0, 0.0, 0.0, 0.0]
            for sj in range(ss):
                for si in range(ss):
                    u = (i + (si + 0.5) / ss) / size
                    v = (j + (sj + 0.5) / ss) / size
                    p = shade(u, v, size)
                    a = p[3] / 255.0
                    acc[0] += p[0] * a
                    acc[1] += p[1] * a
                    acc[2] += p[2] * a
                    acc[3] += p[3]
            n = ss * ss
            alpha = acc[3] / n
            if alpha > 0:
                k = 255.0 / alpha / n
                rows.extend(int(min(255, c * k + 0.5)) for c in acc[:3])
            else:
                rows.extend((0, 0, 0))
            rows.append(int(alpha + 0.5))
    return bytes(rows)


def png(size: int) -> bytes:
    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(render(size), 9))
        + chunk(b"IEND", b"")
    )


def ico(pngs: dict) -> bytes:
    sizes = sorted(pngs)
    header = struct.pack("<HHH", 0, 1, len(sizes))
    offset = 6 + 16 * len(sizes)
    entries, data = b"", b""
    for s in sizes:
        blob = pngs[s]
        dim = 0 if s >= 256 else s
        entries += struct.pack(
            "<BBBBHHII", dim, dim, 0, 0, 1, 32, len(blob), offset + len(data)
        )
        data += blob
    return header + entries + data


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    pngs = {}
    for s in SIZES:
        pngs[s] = png(s)
        (OUT / f"oxystudio-{s}.png").write_bytes(pngs[s])
    (OUT / "oxystudio.ico").write_bytes(ico({s: pngs[s] for s in ICO_SIZES}))
    print("wrote", ", ".join(f"oxystudio-{s}.png" for s in SIZES), "and oxystudio.ico")


if __name__ == "__main__":
    main()
