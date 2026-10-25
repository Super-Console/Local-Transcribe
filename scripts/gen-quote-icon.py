"""Generate the cream / coral quotation-mark app icon and every icon surface.

Source (user-provided): design/app-icon-quote-source.png
  A cream rounded square with two coral quotation marks, 1254x1254.

The square is re-rendered flat in the project's cream (#FAF9F5) with the same
1.5px inner ring as the previous icon, while the quotation glyph keeps the
source's own shading: its alpha is recovered from the coral/cream colour
difference and un-premultiplied, then composited at the measured position.

Outputs (all deterministic; no third-party Python packages):
  icons/icon.png 1024, 32/64/128/256, icon.icns, icon.ico,
  Windows Square*/Store logos, static/favicon.png.

Tray/menu-bar icons are generated separately by scripts/gen-tray-icon.py,
reusing this source quotation glyph as a smaller monochrome template.

Frames are produced at 4x and box-averaged; downscales use ffmpeg's lanczos.
"""

from __future__ import annotations

import importlib.util
import math
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "apps/desktop/src-tauri/icons"
SOURCE = ROOT / "design" / "app-icon-quote-source.png"

spec = importlib.util.spec_from_file_location("tray_png", ROOT / "scripts/gen-tray-icon.py")
png = importlib.util.module_from_spec(spec)
spec.loader.exec_module(png)

CREAM = (250, 249, 245)
INK = (193, 95, 60)
RING = tuple(round(base * 0.93 + dark * 0.07) for base, dark in zip(CREAM, (20, 20, 19)))
RADIUS = 229.0  # same squircle radius as the previous icon (1024 canvas)
SIZE = 1024


def ffmpeg(*args: str) -> None:
    subprocess.run(["ffmpeg", "-v", "error", "-y", *args], check=True)


def source_size(path: Path) -> tuple[int, int]:
    out = subprocess.check_output(
        [
            "ffprobe",
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
            str(path),
        ],
        text=True,
    )
    width, height = out.strip().split(",")
    return int(width), int(height)


def load_rgb(path: Path) -> tuple[int, int, bytes]:
    width, height = source_size(path)
    raw = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(path),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]
    )
    if len(raw) != width * height * 3:
        raise SystemExit(f"unexpected raw size from {path}: {len(raw)}")
    return width, height, raw


def glyph_from_source(path: Path) -> tuple[list, int, int, int, int, int, int]:
    """Return (rgba rows, width, height, bbox) for the coral quotation marks.

    Alpha comes from the red-minus-green difference (coral ≈ 100, cream ≈ 6),
    and the edge colour is un-premultiplied against the cream background so
    the glyph keeps its own shading instead of a dark rim.
    """
    width, height, raw = load_rgb(path)
    alpha = [0] * (width * height)
    for index in range(width * height):
        r, g, b = raw[index * 3 : index * 3 + 3]
        # Saturation separates coral (max-min ≈ 138) from cream (≈ 19) and,
        # unlike the red channel alone, ignores the cream shadows along the glyph.
        chroma = max(r, g, b) - min(r, g, b)
        a = (chroma - 30) / 90
        alpha[index] = 0 if a <= 0 else 255 if a >= 1 else round(a * 255)

    # Bounding box of the glyph.
    xs = [index % width for index, a in enumerate(alpha) if a > 60]
    ys = [index // width for index, a in enumerate(alpha) if a > 60]
    if not xs:
        raise SystemExit("the source has no coral quotation marks")
    left, right = min(xs), max(xs)
    top, bottom = min(ys), max(ys)
    glyph_width = right - left + 1
    glyph_height = bottom - top + 1

    pixels = bytearray()
    for y in range(height):
        for x in range(width):
            index = y * width + x
            a = alpha[index]
            if a == 0:
                pixels.extend((0, 0, 0, 0))
                continue
            r, g, b = raw[index * 3 : index * 3 + 3]
            if a < 250:
                f = a / 255
                r = round(min(255, max(0, (r - CREAM[0] * (1 - f)) / f)))
                g = round(min(255, max(0, (g - CREAM[1] * (1 - f)) / f)))
                b = round(min(255, max(0, (b - CREAM[2] * (1 - f)) / f)))
            pixels.extend((r, g, b, a))
    return [pixels, width, height, left, top, glyph_width, glyph_height]


def sample_bilinear(pixels: bytes, width: int, height: int, x: float, y: float):
    x = min(max(x, 0.0), width - 1.0)
    y = min(max(y, 0.0), height - 1.0)
    x0, y0 = int(x), int(y)
    x1, y1 = min(x0 + 1, width - 1), min(y0 + 1, height - 1)
    fx, fy = x - x0, y - y0
    out = []
    for channel in range(4):
        p00 = pixels[(y0 * width + x0) * 4 + channel]
        p10 = pixels[(y0 * width + x1) * 4 + channel]
        p01 = pixels[(y1 * width + x0) * 4 + channel]
        p11 = pixels[(y1 * width + x1) * 4 + channel]
        top_value = p00 + (p10 - p00) * fx
        bottom_value = p01 + (p11 - p01) * fx
        out.append(top_value + (bottom_value - top_value) * fy)
    return out


def rounded_coverage(x: float, y: float) -> float:
    half = SIZE / 2
    dx = max(abs(x - half) - (half - RADIUS), 0.0)
    dy = max(abs(y - half) - (half - RADIUS), 0.0)
    distance = math.hypot(dx, dy) - RADIUS
    return min(1.0, max(0.0, 0.5 - distance))


def render_master(glyph) -> bytes:
    pixels, gw, gh, gx0, gy0, gwidth, gheight = glyph
    # Measured from the source square (811x839) and reproduced on 1024.
    target_width = 1024 * (gwidth / 811)
    target_height = target_width * (gheight / gwidth)
    center_x = 512.0
    center_y = 512.0 + (631 - 612) / 839 * 1024
    left = center_x - target_width / 2
    top = center_y - target_height / 2

    out = bytearray()
    for y in range(SIZE):
        py = y + 0.5
        for x in range(SIZE):
            px = x + 0.5
            coverage = rounded_coverage(px, py)
            if coverage <= 0.0:
                out.extend((0, 0, 0, 0))
                continue
            base = CREAM
            # 1.5px inward ring, 7% ink, same as the previous icon.
            half = SIZE / 2
            dx = max(abs(px - half) - (half - RADIUS), 0.0)
            dy = max(abs(py - half) - (half - RADIUS), 0.0)
            distance = math.hypot(dx, dy) - RADIUS
            if -2.5 < distance < -0.6:
                base = RING

            # Glyph over the base where the target rect maps to the source bbox.
            r, g, b = base
            if left <= px < left + target_width and top <= py < top + target_height:
                sx = gx0 + (px - left) * (gwidth / target_width) - 0.5
                sy = gy0 + (py - top) * (gheight / target_height) - 0.5
                sr, sg, sb, sa = sample_bilinear(pixels, gw, gh, sx, sy)
                if sa > 0:
                    a = sa / 255
                    r = sr * a + r * (1 - a)
                    g = sg * a + g * (1 - a)
                    b = sb * a + b * (1 - a)
            alpha = round(coverage * 255)
            out.extend((round(r), round(g), round(b), alpha))
    return png.encode_rgba(SIZE, out)


def write_icns(master_path: Path) -> None:
    iconset = Path(tempfile.mkdtemp(prefix="lt-iconset-", suffix=".iconset"))
    try:
        plan = [
            ("icon_16x16.png", 16),
            ("icon_16x16@2x.png", 32),
            ("icon_32x32.png", 32),
            ("icon_32x32@2x.png", 64),
            ("icon_128x128.png", 128),
            ("icon_128x128@2x.png", 256),
            ("icon_256x256.png", 256),
            ("icon_256x256@2x.png", 512),
            ("icon_512x512.png", 512),
            ("icon_512x512@2x.png", 1024),
        ]
        for name, size in plan:
            ffmpeg(
                "-i",
                str(master_path),
                "-vf",
                f"scale={size}:{size}:flags=lanczos",
                str(iconset / name),
            )
        subprocess.run(
            ["iconutil", "-c", "icns", str(iconset), "-o", str(ICONS / "icon.icns")],
            check=True,
        )
    finally:
        shutil.rmtree(iconset, ignore_errors=True)


def write_ico(master_path: Path) -> None:
    sizes = (16, 32, 48, 64, 128, 256)
    payloads = []
    with tempfile.TemporaryDirectory(prefix="lt-ico-") as folder:
        for size in sizes:
            path = Path(folder) / f"{size}.png"
            ffmpeg("-i", str(master_path), "-vf", f"scale={size}:{size}:flags=lanczos", str(path))
            payloads.append(path.read_bytes())
    header = struct.pack("<HHH", 0, 1, len(sizes))
    offset = 6 + 16 * len(sizes)
    entries = bytearray()
    for size, payload in zip(sizes, payloads):
        entries.extend(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(payload), offset))
        offset += len(payload)
    (ICONS / "icon.ico").write_bytes(header + entries + b"".join(payloads))


def main() -> None:
    if not SOURCE.is_file():
        raise SystemExit(f"missing source: {SOURCE}")
    glyph = glyph_from_source(SOURCE)
    master = render_master(glyph)
    master_path = ICONS / "icon.png"
    master_path.write_bytes(master)
    verify_master(master)

    with tempfile.TemporaryDirectory(prefix="lt-icon-sizes-") as folder:
        for size in (32, 64, 128, 256):
            ffmpeg(
                "-i",
                str(master_path),
                "-vf",
                f"scale={size}:{size}:flags=lanczos",
                str(ICONS / ("128x128@2x.png" if size == 256 else f"{size}x{size}.png")),
            )
        # Windows Store logos (Appx) keep the square full-bleed.
        for name, size in {
            "Square30x30Logo.png": 30,
            "Square44x44Logo.png": 44,
            "Square71x71Logo.png": 71,
            "Square89x89Logo.png": 89,
            "Square107x107Logo.png": 107,
            "Square142x142Logo.png": 142,
            "Square150x150Logo.png": 150,
            "Square284x284Logo.png": 284,
            "Square310x310Logo.png": 310,
            "StoreLogo.png": 50,
        }.items():
            ffmpeg(
                "-i",
                str(master_path),
                "-vf",
                f"scale={size}:{size}:flags=lanczos",
                str(ICONS / name),
            )
        write_icns(master_path)
        write_ico(master_path)
        shutil.copy(master_path, ROOT / "apps/desktop/static/favicon.png")

    print(f"glyph {glyph[5]}x{glyph[6]} at ({glyph[3]},{glyph[4]})")
    print(f"master: {master_path} ({len(master)} bytes)")


def verify_master(data: bytes) -> None:
    width, height, pixels = png.decode_rgba(data)
    assert (width, height) == (SIZE, SIZE)
    assert all(pixels[i * 4 + 3] == 0 for i in (0, SIZE - 1, (SIZE - 1) * SIZE, SIZE * SIZE - 1))
    center = ((SIZE // 2) * SIZE + SIZE // 2) * 4
    assert pixels[center + 3] == 255
    # The glyph sits around the middle: some pixel near the centre must be coral.
    found = False
    for y in range(430, 600):
        for x in range(430, 600):
            index = (y * SIZE + x) * 4
            r, g, b = pixels[index : index + 3]
            if r - g > 60 and pixels[index + 3] == 255:
                found = True
                break
        if found:
            break
    assert found, "the quotation glyph is missing from the master"


if __name__ == "__main__":
    main()
