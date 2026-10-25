"""Render source-logo size variants on dark and light backgrounds; evidence only."""
from pathlib import Path
import importlib.util
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "design" / "tray-weight-compare.png"

spec = importlib.util.spec_from_file_location("tray_gen", ROOT / "scripts" / "gen-tray-icon.py")
gen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen)

DARK = (59, 76, 184)    # #3B4CB8, near Timmy's wallpaper
LIGHT = (242, 240, 235)  # #F2F0EB
MAGNIFY = 4
PAD = 24
GAP = 16
SCALES = ("A", "B", "C")  # small / selected / large uniform sizes


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def encode_rgb(width, height, pixels):
    rows = b"".join(b"\0" + pixels[y * width * 3:(y + 1) * width * 3] for y in range(height))
    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def stamp(canvas, cw, ch, icon, size, ox, oy, color):
    """Composite an RGBA icon onto the RGB canvas; template icons are recoloured."""
    _, _, pixels = gen.decode_rgba(icon)
    for y in range(size):
        for x in range(size):
            alpha = pixels[(y * size + x) * 4 + 3]
            if not alpha:
                continue
            base = ((oy + y) * cw + ox + x) * 3
            for channel in range(3):
                blended = (color[channel] * alpha + canvas[base + channel] * (255 - alpha)) // 255
                canvas[base + channel] = blended


def main():
    glyph = gen.load_logo_glyph()
    icons = {scale: gen.render(22, 0, scale, glyph) for scale in SCALES}
    magnified = {scale: gen.render(22 * MAGNIFY, 0, scale, glyph) for scale in SCALES}

    cell = 22
    mag_cell = cell * MAGNIFY
    row_height = mag_cell
    width = PAD * 2 + cell * 3 + GAP * 2 + GAP + mag_cell * 3 + GAP * 2
    height = PAD * 2 + row_height * 2 + GAP
    canvas = bytearray(width * height * 3)

    for row, (background, text) in enumerate([(DARK, (255, 255, 255)), (LIGHT, (0, 0, 0))]):
        top = PAD + row * (row_height + GAP)
        for y in range(top, top + row_height):
            for x in range(PAD, width - PAD):
                canvas[(y * width + x) * 3:(y * width + x) * 3 + 3] = bytes(background)
        for index, scale in enumerate(SCALES):
            ox = PAD + index * (cell + GAP)
            oy = top + (row_height - cell) // 2
            stamp(canvas, width, height, icons[scale], cell, ox, oy, text)
            mag_ox = PAD + cell * 3 + GAP * 2 + GAP + index * (mag_cell + GAP)
            stamp(canvas, width, height, magnified[scale], mag_cell, mag_ox, top, text)

    data = encode_rgb(width, height, canvas)
    OUT.write_bytes(data)
    if data[:8] != b"\x89PNG\r\n\x1a\n" or struct.unpack(">II", data[16:24]) != (width, height):
        raise ValueError("comparison sheet failed PNG round-trip")
    print(f"{OUT.name}: {width}x{height}; scales {SCALES}; dark {DARK} / light {LIGHT}")


if __name__ == "__main__":
    main()
