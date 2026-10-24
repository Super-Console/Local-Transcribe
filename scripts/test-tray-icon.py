"""Tray resources for the thin quote design; structural checks only.

Taskbook "菜单栏引号图标（细体）" §5: on tray-template@2x.png exactly two
connected components, bounding box 16-19 x 20-22px centred, every top-quarter
horizontal run <=4px, opaque ink 3-6% of the canvas. No appearance judgement.
"""
import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "apps/desktop/src-tauri/icons"


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def runs(pixels, size, y, threshold=0):
    segments, start = [], None
    for x in range(size):
        alpha = pixels[(y * size + x) * 4 + 3]
        if alpha > threshold and start is None:
            start = x
        elif alpha <= threshold and start is not None:
            segments.append((start, x - 1))
            start = None
    if start is not None:
        segments.append((start, size - 1))
    return segments


def components(pixels, size):
    alpha = [pixels[index * 4 + 3] for index in range(size * size)]
    seen, count = set(), 0
    for index, value in enumerate(alpha):
        if value == 0 or index in seen:
            continue
        count += 1
        seen.add(index)
        stack = [index]
        while stack:
            current = stack.pop()
            cy, cx = divmod(current, size)
            for ny in range(cy - 1, cy + 2):
                for nx in range(cx - 1, cx + 2):
                    other = ny * size + nx
                    if 0 <= ny < size and 0 <= nx < size and other not in seen and alpha[other]:
                        seen.add(other)
                        stack.append(other)
    return count


def bounding_box(pixels, size):
    points = [(index % size, index // size) for index in range(size * size) if pixels[index * 4 + 3]]
    xs = [x for x, _ in points]
    ys = [y for _, y in points]
    return min(xs), max(xs), min(ys), max(ys)


class TrayIconTests(unittest.TestCase):
    def setUp(self):
        self.module = load("gen-tray-icon")

    def read(self, name):
        return self.module.decode_rgba((ICONS / name).read_bytes())

    def test_files_sizes_and_transparency(self):
        for name, size in [("tray-template.png", 22), ("tray-template@2x.png", 44), ("tray-windows.png", 32)]:
            width, height, pixels = self.read(name)
            self.assertEqual((width, height), (size, size), name)
            for index in (0, size - 1, (size - 1) * size, size * size - 1):
                self.assertEqual(pixels[index * 4 + 3], 0, f"{name} corner")

    def test_two_quote_marks_only(self):
        width, _, pixels = self.read("tray-template@2x.png")
        self.assertEqual(components(pixels, width), 2)

    def test_bounding_box_and_centring(self):
        width, _, pixels = self.read("tray-template@2x.png")
        x0, x1, y0, y1 = bounding_box(pixels, width)
        box_width, box_height = x1 - x0 + 1, y1 - y0 + 1
        self.assertGreaterEqual(box_width, 16)
        self.assertLessEqual(box_width, 19)
        self.assertGreaterEqual(box_height, 20)
        self.assertLessEqual(box_height, 22)
        self.assertLessEqual(abs(x0 - (width - 1 - x1)), 1)
        self.assertLessEqual(abs(y0 - (width - 1 - y1)), 1)

    def test_top_quarter_strokes_are_thin(self):
        width, _, pixels = self.read("tray-template@2x.png")
        x0, x1, y0, y1 = bounding_box(pixels, width)
        y_scan = y0 + (y1 - y0) // 4
        segments = runs(pixels, width, y_scan)
        self.assertEqual(len(segments), 2, f"expected one stroke per mark at y={y_scan}, got {segments}")
        for start, end in segments:
            self.assertLessEqual(end - start + 1, 4, f"stroke too thick at y={y_scan}: {segments}")

    def test_opaque_ink_ratio(self):
        width, _, pixels = self.read("tray-template@2x.png")
        opaque = sum(1 for index in range(3, len(pixels), 4) if pixels[index] == 255)
        ratio = opaque / (width * width)
        self.assertGreaterEqual(ratio, 0.03)
        self.assertLessEqual(ratio, 0.06)

    def test_macos_template_is_black(self):
        for name in ("tray-template.png", "tray-template@2x.png"):
            width, _, pixels = self.read(name)
            for index in range(0, len(pixels), 4):
                if pixels[index + 3]:
                    self.assertEqual(tuple(pixels[index:index + 3]), (0, 0, 0), name)

    def test_windows_same_shape_in_white(self):
        width, _, pixels = self.read("tray-windows.png")
        self.assertEqual(components(pixels, width), 2)
        x0, x1, y0, y1 = bounding_box(pixels, width)
        # 32/22 of the 8.5 x 10.5pt group, allow a pixel of anti-aliasing.
        self.assertLessEqual(abs((x1 - x0 + 1) - 12), 2)
        self.assertLessEqual(abs((y1 - y0 + 1) - 15), 2)
        for index in range(0, len(pixels), 4):
            if pixels[index + 3] == 255:
                self.assertTrue(all(pixels[index + channel] >= 240 for channel in range(3)))

    def test_generation_is_repeatable(self):
        names = ["tray-template.png", "tray-template@2x.png", "tray-windows.png"]
        before = {name: (ICONS / name).read_bytes() for name in names}
        subprocess.run(
            [sys.executable, str(ROOT / "scripts/gen-tray-icon.py")],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        for name in names:
            self.assertEqual((ICONS / name).read_bytes(), before[name], name)

    def test_platform_loading_contract(self):
        source = (ROOT / "apps/desktop/src-tauri/src/tray.rs").read_text()
        self.assertIn('include_bytes!("../icons/tray-template.png")', source)
        self.assertIn('include_bytes!("../icons/tray-windows.png")', source)
        self.assertIn('icon_as_template(cfg!(target_os = "macos"))', source)
        self.assertNotIn("default_window_icon", source)


if __name__ == "__main__":
    unittest.main()
