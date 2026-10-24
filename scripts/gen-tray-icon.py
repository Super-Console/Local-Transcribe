"""Generate the quote-mark tray icons using only the Python standard library.

Geometry is defined once in 22pt design units (taskbook "菜单栏引号图标（取中）"):
canvas 22x22, a centred quote group; each mark occupies a box, the two boxes are
separated by MARK_GAP. A mark is a solid comma: a disc-swept Bezier centreline
whose radius tapers from head to tail.

The stroke weight comes in three grades measured at the @2x mid-canvas:
A=3.0px, B=3.5px (the shipped grade), C=4.0px. All three share one parameter
table; the grade only scales the radius profile.

Each output size is drawn at 4x as the union of many discs along that curve,
then box-averaged. RGB is constant black for the macOS template images and
white for Windows; the background stays transparent. Run from any directory;
output is deterministic and validated after writing.
"""
from pathlib import Path
import struct
import zlib

ICONS = Path(__file__).resolve().parents[1] / "apps/desktop/src-tauri/icons"
SUPERSAMPLE = 4
CANVAS = 22.0
MARK_BOX_WIDTH = 3.5
MARK_GAP = 0.5
GROUP_WIDTH = 2 * MARK_BOX_WIDTH + MARK_GAP
GROUP_TOP = (CANVAS - 10.5) / 2  # 5.75
GROUP_LEFT = (CANVAS - GROUP_WIDTH) / 2
# Cubic centreline (box-local pt) and radius profile for one comma mark.
HEAD = (2.55, 1.05)
CTRL_1 = (2.6, 4.6)
CTRL_2 = (1.9, 8.2)
TAIL = (0.4, 10.0)
RADIUS_START = 1.05
RADIUS_TAPER = 0.72  # r(t) = RADIUS_START - RADIUS_TAPER * t ** 0.7
RADIUS_END = 0.33
SWEEP_SAMPLES = 600
# Stroke grades in @2x px (taskbook §3); the shipped files use GRADE_B.
GRADES = {"A": 3.0, "B": 3.5, "C": 4.0}
GRADE_B = "B"
_GRADE_SCALE = {name: px / GRADES[GRADE_B] for name, px in GRADES.items()}


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


def _radius(t, grade):
    scale = _GRADE_SCALE[grade]
    return (RADIUS_START - RADIUS_TAPER * t ** 0.7) * scale


def _centre(t):
    u = 1.0 - t
    x = (u ** 3 * HEAD[0] + 3 * u * u * t * CTRL_1[0]
         + 3 * u * t * t * CTRL_2[0] + t ** 3 * TAIL[0])
    y = (u ** 3 * HEAD[1] + 3 * u * u * t * CTRL_1[1]
         + 3 * u * t * t * CTRL_2[1] + t ** 3 * TAIL[1])
    return x, y


def _paint_mark(coverage, size, box_left, unit, grade):
    """Union of discs along one comma centreline, in the supersampled buffer."""
    for index in range(SWEEP_SAMPLES + 1):
        t = index / SWEEP_SAMPLES
        cx, cy = _centre(t)
        cx = (box_left + cx) * unit
        cy = (GROUP_TOP + cy) * unit
        radius = _radius(t, grade) * unit
        reach = int(radius) + 1
        x0, x1 = int(cx) - reach, int(cx) + reach
        y0, y1 = int(cy) - reach, int(cy) + reach
        for y in range(max(0, y0), min(size, y1 + 1)):
            dy = y + 0.5 - cy
            row = y * size
            for x in range(max(0, x0), min(size, x1 + 1)):
                dx = x + 0.5 - cx
                if dx * dx + dy * dy <= radius * radius:
                    coverage[row + x] = 1


def render(size, color, grade=GRADE_B):
    """Draw the same 22pt quote geometry at any square pixel size."""
    scale = size / CANVAS
    high = size * SUPERSAMPLE
    unit = SUPERSAMPLE * scale  # high-res pixels per design point
    coverage = bytearray(high * high)
    for box_index in range(2):
        _paint_mark(coverage, high, GROUP_LEFT + box_index * (MARK_BOX_WIDTH + MARK_GAP), unit, grade)
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
            pixels.extend((color, color, color, (total * 255 + samples // 2) // samples))
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
    if size == 44 and not 0.026 <= opaque / (size * size) <= 0.040:
        # Taskbook §4's 2.6-4.0% ink band is defined on the @2x template; 1x
        # has too few pixels to contain a fully opaque 1.75pt stroke.
        raise ValueError(f"{name}: ink {opaque / (size * size):.1%} outside 2.6-4.0%")
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
