"""Generate the three-bar tray icons using only the Python standard library.

Geometry is defined once in 22pt design units (taskbook "菜单栏图标重做"):
canvas 22x22, content vertically centred on y=11, three rounded vertical bars
1.5pt wide with 3pt gaps, heights 6.5 / 11 / 8.5, cap radius 0.75; the whole
group is 10.5pt wide and horizontally centred.

Each output size is drawn at 4x and box-averaged. RGB is constant black for the
macOS template images and white for Windows; the background stays transparent.
Run from any directory; output is deterministic and validated after writing.
"""
from pathlib import Path
import struct
import zlib

ICONS = Path(__file__).resolve().parents[1] / "apps/desktop/src-tauri/icons"
SUPERSAMPLE = 4
CANVAS = 22.0
BAR_WIDTH = 1.5
GAP = 3.0
HEIGHTS = (6.5, 11.0, 8.5)
CAP_RADIUS = 0.75
CENTER_Y = 11.0
GROUP_WIDTH = 3 * BAR_WIDTH + 2 * GAP  # 10.5
LEFT = (CANVAS - GROUP_WIDTH) / 2  # 5.75


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
    """Draw the same 22pt geometry at any square pixel size."""
    scale = size / CANVAS
    high = size * SUPERSAMPLE
    points_per_sample = 1.0 / (SUPERSAMPLE * scale)
    coverage = bytearray(high * high)
    for y in range(high):
        py = (y + 0.5) * points_per_sample
        for x in range(high):
            px = (x + 0.5) * points_per_sample
            for index, height in enumerate(HEIGHTS):
                left = LEFT + index * (BAR_WIDTH + GAP)
                cx = left + BAR_WIDTH / 2
                top = CENTER_Y - height / 2
                nearest_y = min(max(py, top + CAP_RADIUS), top + height - CAP_RADIUS)
                if (px - cx) ** 2 + (py - nearest_y) ** 2 <= CAP_RADIUS ** 2:
                    coverage[y * high + x] = 255
                    break
    pixels = bytearray()
    for y in range(size):
        for x in range(size):
            total = sum(coverage[(y * SUPERSAMPLE + dy) * high + x * SUPERSAMPLE + dx]
                        for dy in range(SUPERSAMPLE) for dx in range(SUPERSAMPLE))
            pixels.extend((color, color, color, (total + 8) // 16))
    return encode_rgba(size, pixels)


def verify(data, size, name):
    width, height, pixels = decode_rgba(data)
    if (width, height) != (size, size):
        raise ValueError(f"{name}: wrong dimensions")
    corners = (0, size - 1, (size - 1) * size, size * size - 1)
    if any(pixels[i * 4 + 3] != 0 for i in corners):
        raise ValueError(f"{name}: nontransparent corner")
    if not any(pixels[i * 4 + 3] == 255 for i in range(0, len(pixels), 4)):
        raise ValueError(f"{name}: no opaque pixel")
    opaque = sum(1 for i in range(3, len(pixels), 4) if pixels[i] == 255)
    if opaque / (size * size) > 0.12:
        raise ValueError(f"{name}: too much ink ({opaque / (size * size):.1%})")
    if size in (22, 44):
        if any(tuple(pixels[i:(i + 3)]) != (0, 0, 0) for i in range(0, len(pixels), 4) if pixels[i + 3]):
            raise ValueError(f"{name}: macOS template must be pure black")
    print(f"{name}: {width}x{height} RGBA; corners transparent; opaque {opaque / (size * size):.1%}")


def main():
    for name, size, color in [("tray-template.png", 22, 0),
                              ("tray-template@2x.png", 44, 0),
                              ("tray-windows.png", 32, 255)]:
        data = render(size, color)
        verify(data, size, name)
        path = ICONS / name
        path.write_bytes(data)
        verify(path.read_bytes(), size, name)


if __name__ == "__main__":
    main()
