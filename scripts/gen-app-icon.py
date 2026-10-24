"""LEGACY waveform icon generator (superseded by gen-quote-icon.py).

Running this file rewrites icons/icon.* with the old waveform design. The
current icon is the quote-mark design; use scripts/gen-quote-icon.py unless you
are deliberately restoring the A waveform. Requires --legacy-waveform.

Original purpose: generate approved A application icons only, using standard-library 4x rasterization.

Base geometry is from the taskbook, with the user's taller-wave amendment:
168/384/240px heights; 229px canvas radius,
1.5px inward ring, three 62px waves plus one separate 226x62px line.
Never changes tray assets, B proposals, the A reference, or app versions.
"""
from array import array
import importlib.util
import math
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "apps/desktop/src-tauri/icons"
spec = importlib.util.spec_from_file_location("proposal_geometry", ROOT / "scripts/gen-app-icon-b.py")
geometry = importlib.util.module_from_spec(spec)
spec.loader.exec_module(geometry)
png = geometry.png
BG = (250, 249, 245)
INK = (193, 95, 60)
RING = tuple(round(b * .93 + f * .07) for b, f in zip(BG, (20, 20, 19)))
SHAPES = [(232, 428, 62, 168, 31), (338, 320, 62, 384, 31),
          (444, 392, 62, 240, 31), (566, 481, 226, 62, 31)]


def render(size):
    high = size * 4
    step = 1024 / high
    sums = array("I", [0]) * (size * size * 4)
    for hy in range(high):
        y = (hy + .5) * step
        outer = geometry.span((0, 0, 1024, 1024, 229), y)
        inner = geometry.span((1.5, 1.5, 1021, 1021, 227.5), y)
        if outer is None:
            continue
        glyph = [s for rect in SHAPES if (s := geometry.span(rect, y)) is not None]
        first = max(0, math.ceil(outer[0] / step - .5))
        last = min(high, math.ceil(outer[1] / step - .5))
        for hx in range(first, last):
            x = (hx + .5) * step
            color = INK if any(a <= x < b for a, b in glyph) else (
                BG if inner is not None and inner[0] <= x < inner[1] else RING)
            i = ((hy // 4) * size + hx // 4) * 4
            for k in range(3):
                sums[i + k] += color[k]
            sums[i + 3] += 1
    pixels = bytearray()
    for i in range(0, len(sums), 4):
        n = sums[i + 3]
        pixels.extend((*(round(sums[i + k] / n) for k in range(3)), round(n * 255 / 16)) if n else (0, 0, 0, 0))
    return png.encode_rgba(size, pixels)


def validate(images):
    w, h, p = png.decode_rgba(images[1024])
    assert (w, h) == (1024, 1024)
    assert all(p[i * 4 + 3] == 0 for i in [0, 1023, 1024 * 1023, 1024 * 1024 - 1])
    for x, y, expected in [(263, 512, INK), (679, 512, INK), (512, 512, BG)]:
        assert tuple(p[(y * w + x) * 4:(y * w + x) * 4 + 3]) == expected
    _, _, small = png.decode_rgba(images[32])
    exact = sum(tuple(small[i:i + 3]) == INK for i in range(0, len(small), 4))
    covered = sum(small[i] < BG[0] and small[i + 1] < BG[1] - 10 and small[i + 2] < BG[2] - 10
                  for i in range(0, len(small), 4) if small[i + 3] == 255)
    print(f"32px terracotta: exact={exact}, including antialiased coverage={covered}")
    assert covered > 60, "32px foreground coverage must exceed 60 pixels"


def main():
    scratch = Path(tempfile.mkdtemp(prefix="app-icon-a-", dir=Path(tempfile.gettempdir()) / "opencode"))
    images = {size: render(size) for size in [1024, 512, 256, 128, 64, 48, 32, 16]}
    validate(images)
    for size, data in images.items():
        (scratch / f"a-{size}.png").write_bytes(data)
    iconset = scratch / "LocalTranscribe.iconset"
    iconset.mkdir()
    for base in [16, 32, 128, 256, 512]:
        for factor, suffix in [(1, ""), (2, "@2x")]:
            subprocess.run(["sips", "-s", "format", "png", str(scratch / f"a-{base * factor}.png"),
                            "--out", str(iconset / f"icon_{base}x{base}{suffix}.png")], check=True, stdout=subprocess.DEVNULL)
    icns = scratch / "icon.icns"
    subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(icns)], check=True)
    ico_sizes = [16, 32, 48, 64, 128, 256]
    offset = 6 + 16 * len(ico_sizes)
    entries = []
    for size in ico_sizes:
        entries.append(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(images[size]), offset))
        offset += len(images[size])
    ico = struct.pack("<HHH", 0, 1, len(entries)) + b"".join(entries) + b"".join(images[s] for s in ico_sizes)
    for name, size in {"icon.png": 1024, "128x128.png": 128, "128x128@2x.png": 256,
                       "32x32.png": 32, "64x64.png": 64}.items():
        (ICONS / name).write_bytes(images[size])
    (ICONS / "icon.icns").write_bytes(icns.read_bytes())
    (ICONS / "icon.ico").write_bytes(ico)
    assert (ICONS / "icon.icns").stat().st_size and (ICONS / "icon.ico").stat().st_size
    print(f"A resources generated; retained intermediate files: {scratch}")


if __name__ == "__main__":
    if "--legacy-waveform" not in sys.argv:
        raise SystemExit(
            "This is the legacy waveform generator; run scripts/gen-quote-icon.py instead. "
            "Pass --legacy-waveform to overwrite icons with the old design."
        )
    main()
