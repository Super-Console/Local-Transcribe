"""Generate menu-bar silhouettes from the exact quotation glyph in the app logo.

The app icon's extracted alpha mask is uniformly scaled to a 12.5pt maximum
width on a 22pt canvas. No redraw, reshaping, or independent stroke adjustment
is applied. A/B/C are smaller-to-larger uniform-size comparison variants; B is
the shipped size. The source artwork has a fuller silhouette than the former
hand-drawn stroke, so B is deliberately between the original small version and
the visually heavy largest option. RGB is black for macOS templates and white
for Windows.
"""
from functools import lru_cache
import importlib.util
from pathlib import Path
import struct
import zlib

ICONS = Path(__file__).resolve().parents[1] / "apps/desktop/src-tauri/icons"
ROOT = Path(__file__).resolve().parents[1]
LOGO_SOURCE = ROOT / "design/app-icon-quote-source.png"
SUPERSAMPLE = 4
CANVAS = 22.0
# Uniform scaling variants in 22pt design units; B balances the source glyph's
# fuller silhouette against the menu-bar neighbors without distorting its shape.
SCALE_WIDTHS = {"A": 10.5, "B": 12.5, "C": 14.5}
SCALE_B = "B"


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def encode_rgba(size, pixels):
    rows = b"".join(b"\0" + pixels[y * size * 4:(y + 1) * size * 4] for y in range(size))
    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def decode_rgba(data):
    """Read back our non-interlaced RGBA PNG, including chunk CRC checks."""
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("Not a PNG")
    offset, compressed, size = 8, bytearray(), None
    while offset < len(data):
        length = struct.unpack_from(">I", data, offset)[0]
        kind = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + length]
        crc = struct.unpack_from(">I", data, offset + 8 + length)[0]
        if zlib.crc32(kind + payload) != crc:
            raise ValueError("PNG chunk CRC mismatch")
        if kind == b"IHDR":
            width, height, depth, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", payload)
            if (depth, color, compression, filtering, interlace) != (8, 6, 0, 0, 0):
                raise ValueError("Expected non-interlaced 8-bit RGBA PNG")
            size = width, height
        elif kind == b"IDAT":
            compressed.extend(payload)
        offset += length + 12
        if kind == b"IEND":
            break
    if size is None:
        raise ValueError("Missing PNG dimensions")
    width, height = size
    raw = zlib.decompress(compressed)
    stride = width * 4 + 1
    if len(raw) != stride * height or any(raw[y * stride] != 0 for y in range(height)):
        raise ValueError("Invalid row length or unsupported PNG filter")
    pixels = b"".join(raw[y * stride + 1:(y + 1) * stride] for y in range(height))
    return width, height, pixels


@lru_cache(maxsize=1)
def load_logo_glyph():
    """Use the existing app-icon extractor so the tray shape is not redrawn."""
    spec = importlib.util.spec_from_file_location("quote_icon_source", ROOT / "scripts/gen-quote-icon.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.glyph_from_source(LOGO_SOURCE)


def _sample_alpha(pixels, width, height, x, y):
    x = min(max(x, 0.0), width - 1.0)
    y = min(max(y, 0.0), height - 1.0)
    x0, y0 = int(x), int(y)
    x1, y1 = min(x0 + 1, width - 1), min(y0 + 1, height - 1)
    fx, fy = x - x0, y - y0
    a00 = pixels[(y0 * width + x0) * 4 + 3]
    a10 = pixels[(y0 * width + x1) * 4 + 3]
    a01 = pixels[(y1 * width + x0) * 4 + 3]
    a11 = pixels[(y1 * width + x1) * 4 + 3]
    top = a00 + (a10 - a00) * fx
    bottom = a01 + (a11 - a01) * fx
    return top + (bottom - top) * fy


def render(size, color, scale=SCALE_B, glyph=None):
    """Uniformly scale the source logo mask onto a square icon canvas."""
    if scale not in SCALE_WIDTHS:
        raise ValueError(f"unknown logo scale: {scale}")
    glyph = glyph or load_logo_glyph()
    source_pixels, source_width, source_height, gx, gy, glyph_width, glyph_height = glyph
    target_width = SCALE_WIDTHS[scale]
    target_height = target_width * glyph_height / glyph_width
    left = (CANVAS - target_width) / 2
    top = (CANVAS - target_height) / 2
    high = size * SUPERSAMPLE
    design_units_per_sample = CANVAS / high
    coverage = bytearray(high * high)

    for y in range(high):
        py = (y + 0.5) * design_units_per_sample
        if py < top or py >= top + target_height:
            continue
        sy = gy + (py - top) * glyph_height / target_height - 0.5
        row = y * high
        for x in range(high):
            px = (x + 0.5) * design_units_per_sample
            if px < left or px >= left + target_width:
                continue
            sx = gx + (px - left) * glyph_width / target_width - 0.5
            coverage[row + x] = round(_sample_alpha(source_pixels, source_width, source_height, sx, sy))

    pixels = bytearray()
    samples = SUPERSAMPLE * SUPERSAMPLE
    for y in range(size):
        for x in range(size):
            total = 0
            base_y = y * SUPERSAMPLE * high
            for dy in range(SUPERSAMPLE):
                row = base_y + dy * high + x * SUPERSAMPLE
                for dx in range(SUPERSAMPLE):
                    total += coverage[row + dx]
            pixels.extend((color, color, color, (total + samples // 2) // samples))
    return encode_rgba(size, pixels)


def verify(data, size, name):
    width, height, pixels = decode_rgba(data)
    if (width, height) != (size, size):
        raise ValueError(f"{name}: wrong dimensions")
    corners = (0, size - 1, (size - 1) * size, size * size - 1)
    if any(pixels[i * 4 + 3] != 0 for i in corners):
        raise ValueError(f"{name}: nontransparent corner")
    if not any(pixels[i + 3] == 255 for i in range(0, len(pixels), 4)):
        raise ValueError(f"{name}: no opaque pixel")
    opaque = sum(1 for i in range(3, len(pixels), 4) if pixels[i] == 255)
    if size in (22, 44):
        if any(tuple(pixels[i:(i + 3)]) != (0, 0, 0) for i in range(0, len(pixels), 4) if pixels[i + 3]):
            raise ValueError(f"{name}: macOS template must be pure black")
    print(f"{name}: {width}x{height} RGBA; corners transparent; opaque {opaque / (size * size):.1%}")


def main():
    glyph = load_logo_glyph()
    for name, size, color in [("tray-template.png", 22, 0),
                              ("tray-template@2x.png", 44, 0),
                              ("tray-windows.png", 32, 255)]:
        data = render(size, color, glyph=glyph)
        verify(data, size, name)
        path = ICONS / name
        path.write_bytes(data)
        verify(path.read_bytes(), size, name)


if __name__ == "__main__":
    main()
