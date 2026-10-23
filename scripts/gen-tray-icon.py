"""Generate transparent waveform tray PNGs using only the Python standard library.

Draw at 4x the output resolution, then box-average alpha. RGB is constant
black (macOS template) or white (Windows); no opaque background is drawn.
Run from any directory; output is deterministic and validated after writing.
"""
from pathlib import Path
import struct
import zlib

ICONS = Path(__file__).resolve().parents[1] / "apps/desktop/src-tauri/icons"
SUPERSAMPLE = 4


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


def render(size, color):
    # The requested x starts are 3, 9, 15 in 22px coordinates; scale the
    # entire 22px design proportionally for Retina and Windows resources.
    scale = size / 22
    high = size * SUPERSAMPLE
    coverage = bytearray(high * high)
    # Only macOS black templates receive the taller-wave amendment.
    # Preserve Windows geometry and bytes exactly.
    heights = (8, 17, 11) if color == 0 else (7, 13, 9)
    for y in range(high):
        py = (y + 0.5) / (SUPERSAMPLE * scale)
        for x in range(high):
            px = (x + 0.5) / (SUPERSAMPLE * scale)
            for left, height in zip((3, 9, 15), heights):
                cx = left + 1.5
                top = 11 - height / 2
                nearest_y = min(max(py, top + 1.5), top + height - 1.5)
                if (px - cx) ** 2 + (py - nearest_y) ** 2 <= 1.5 ** 2:
                    coverage[y * high + x] = 255
                    break
    pixels = bytearray()
    for y in range(size):
        for x in range(size):
            total = sum(coverage[(y * 4 + dy) * high + x * 4 + dx]
                        for dy in range(4) for dx in range(4))
            pixels.extend((color, color, color, (total + 8) // 16))
    return encode_rgba(size, pixels)


def main():
    for name, size, color in [("tray-template.png", 22, 0),
                              ("tray-template@2x.png", 44, 0),
                              ("tray-windows.png", 32, 255)]:
        path = ICONS / name
        path.write_bytes(render(size, color))
        width, height, pixels = decode_rgba(path.read_bytes())
        if (width, height) != (size, size):
            raise ValueError(f"{name}: wrong dimensions")
        corners = (0, size - 1, (size - 1) * size, size * size - 1)
        if any(pixels[i * 4 + 3] != 0 for i in corners):
            raise ValueError(f"{name}: nontransparent corner")
        if not any(pixels[(y * size + size // 2) * 4 + 3] == 255 for y in range(size)):
            raise ValueError(f"{name}: no opaque center-column pixel")
        print(f"{name}: {width}x{height} RGBA; corners alpha=0; center alpha=255")


if __name__ == "__main__":
    main()
