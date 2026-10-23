"""Generate B-only application icon proposals; never write icons/ or the A reference.

Thesis: three waves settle onto a shared baseline; the last wave continues
right into one text line. Two conceptual elements, one flat terracotta ink.
1024 geometry: radius229, ivory #FAF9F5, terracotta #C15F3C; no effects/ring.
Wave rectangles: x208/320/432, width64, heights288/224/160, bottom656.
Text line: x432..816, y592..656. Rounded ends radius32 throughout.
Render each requested size at 4x, then box-average premultiplied RGBA.
The existing gen-app-icon.py is absent; reuse the tray standard-library codec.
"""
from array import array
import hashlib
import importlib.util
import math
from pathlib import Path
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("tray_png", ROOT / "scripts/gen-tray-icon.py")
png = importlib.util.module_from_spec(spec)
spec.loader.exec_module(png)
BACKGROUND = (250, 249, 245)
INK = (193, 95, 60)
SAMPLES = 4
WAVES = [(208, 368, 64, 288, 32), (320, 432, 64, 224, 32), (432, 496, 64, 160, 32)]
LINE = (432, 592, 384, 64, 32)


def span(rect, y):
    left, top, width, height, radius = rect
    if not top <= y < top + height:
        return None
    cy = min(max(y, top + radius), top + height - radius)
    dx = math.sqrt(max(0, radius * radius - (y - cy) ** 2))
    return left + radius - dx, left + width - radius + dx


def render(size):
    high = size * SAMPLES
    step = 1024 / high
    sums = array("I", [0]) * (size * size * 4)
    for hy in range(high):
        y = (hy + 0.5) * step
        edge = span((0, 0, 1024, 1024, 229), y)
        if edge is None:
            continue
        glyph = [s for rect in [*WAVES, LINE] if (s := span(rect, y)) is not None]
        first = max(0, math.ceil(edge[0] / step - 0.5))
        last = min(high, math.ceil(edge[1] / step - 0.5))
        for hx in range(first, last):
            x = (hx + 0.5) * step
            color = INK if any(a <= x < b for a, b in glyph) else BACKGROUND
            i = ((hy // SAMPLES) * size + hx // SAMPLES) * 4
            sums[i] += color[0]
            sums[i + 1] += color[1]
            sums[i + 2] += color[2]
            sums[i + 3] += 1
    pixels = bytearray()
    for i in range(0, len(sums), 4):
        count = sums[i + 3]
        if count:
            pixels.extend((*(round(sums[i + k] / count) for k in range(3)), round(count * 255 / 16)))
        else:
            pixels.extend((0, 0, 0, 0))
    return png.encode_rgba(size, pixels)


def verify(data, size):
    width, height, pixels = png.decode_rgba(data)
    assert (width, height) == (size, size)
    assert all(pixels[i * 4 + 3] == 0 for i in [0, size - 1, size * (size - 1), size * size - 1])
    assert all(pixels[i + 3] == 255 for i in range(0, len(pixels), 4) if tuple(pixels[i:i + 3]) == INK)
    if size == 32:
        # Structural checks only; human recognition remains the user's decision.
        def ink(x, y):
            i = (y * size + x) * 4
            return tuple(pixels[i:i + 3]) == INK and pixels[i + 3] == 255
        assert all(ink(x, 16) for x in [7, 10, 14]), "three distinct wave cores"
        assert not ink(9, 16) and not ink(12, 16), "gaps between waves remain"
        assert all(ink(x, 19) for x in range(14, 25)), "one continuous horizontal core"
        print("32px structure: wave width=2px, gaps=1.5px, heights=9/7/5px; line=12x2px")


def main():
    outputs = [("app-icon-b.png", 1024), ("app-icon-b-128.png", 128), ("app-icon-b-32.png", 32)]
    for name, size in outputs:
        data = render(size)
        verify(data, size)
        path = ROOT / "design" / name
        path.write_bytes(data)
        verify(path.read_bytes(), size)
        print(f"{name}: {size}x{size} RGBA SHA256={hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
