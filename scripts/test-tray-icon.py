"""Tray resources for the middle-weight quote design; structural checks only.

Taskbook "菜单栏引号图标（取中）" §4 on the grade-B tray-template@2x.png:
exactly two connected components, bounding box 16-18 x 21-23px centred,
thickest vertical stroke 3.0-4.0px at the top-quarter scan, opaque ink
2.6-4.0% of the canvas. No appearance judgement.
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


def runs(pixels, size, y, threshold=128):
    segments, start = [], None
    for x in range(size):
        alpha = pixels[(y * size + x) * 4 + 3]
        if alpha >= threshold and start is None:
            start = x
        elif alpha < threshold and start is not None:
            segments.append((start, x - 1))
            start = None
    if start is not None:
        segments.append((start, size - 1))
    return segments


def components(pixels, size):
    alpha = [pixels[index * 4 + 3] for index in range(size * size)]
    seen, groups = set(), []
    for index, value in enumerate(alpha):
        if value == 0 or index in seen:
            continue
        blob, stack = [], [index]
        seen.add(index)
        while stack:
            current = stack.pop()
            blob.append(current)
            cy, cx = divmod(current, size)
            for ny in range(cy - 1, cy + 2):
                for nx in range(cx - 1, cx + 2):
                    other = ny * size + nx
                    if 0 <= ny < size and 0 <= nx < size and other not in seen and alpha[other]:
                        seen.add(other)
                        stack.append(other)
        groups.append(blob)
    return groups


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

    def test_exactly_two_quote_marks(self):
        width, _, pixels = self.read("tray-template@2x.png")
        self.assertEqual(len(components(pixels, width)), 2)

    def test_bounding_box_and_centring(self):
        width, _, pixels = self.read("tray-template@2x.png")
        x0, x1, y0, y1 = bounding_box(pixels, width)
        box_width, box_height = x1 - x0 + 1, y1 - y0 + 1
        self.assertGreaterEqual(box_width, 16)
        self.assertLessEqual(box_width, 18)
        self.assertGreaterEqual(box_height, 21)
        self.assertLessEqual(box_height, 23)
        self.assertLessEqual(abs(x0 - (width - 1 - x1)), 1)

    def test_vertical_stroke_weight(self):
        width, _, pixels = self.read("tray-template@2x.png")
        x0, x1, y0, y1 = bounding_box(pixels, width)
        y_scan = y0 + (y1 - y0) // 4
        segments = runs(pixels, width, y_scan)
        self.assertEqual(len(segments), 2, f"expected one stroke per mark at y={y_scan}, got {segments}")
        for start, end in segments:
            thickness = end - start + 1
            self.assertGreaterEqual(thickness, 3.0, f"stroke too thin at y={y_scan}: {segments}")
            self.assertLessEqual(thickness, 4.0, f"stroke too thick at y={y_scan}: {segments}")

    def test_mark_gap(self):
        width, _, pixels = self.read("tray-template@2x.png")
        x0, x1, _, _ = bounding_box(pixels, width)
        groups = components(pixels, width)
        self.assertEqual(len(groups), 2)
        left, right = sorted(groups, key=lambda blob: min(i % width for i in blob))
        gap = min((((k % width) - (j % width)) ** 2 + ((k // width) - (j // width)) ** 2) ** 0.5 - 1
                  for j in left for k in right)
        self.assertGreaterEqual(gap, 3)
        self.assertLessEqual(gap, 4)

    def test_opaque_ink_ratio(self):
        width, _, pixels = self.read("tray-template@2x.png")
        opaque = sum(1 for index in range(3, len(pixels), 4) if pixels[index] == 255)
        ratio = opaque / (width * width)
        self.assertGreaterEqual(ratio, 0.026)
        self.assertLessEqual(ratio, 0.040)

    def test_macos_template_is_black(self):
        for name in ("tray-template.png", "tray-template@2x.png"):
            width, _, pixels = self.read(name)
            for index in range(0, len(pixels), 4):
                if pixels[index + 3]:
                    self.assertEqual(tuple(pixels[index:index + 3]), (0, 0, 0), name)

    def test_windows_same_shape_in_white(self):
        width, _, pixels = self.read("tray-windows.png")
        self.assertEqual(len(components(pixels, width)), 2)
        x0, x1, y0, y1 = bounding_box(pixels, width)
        # 32/22 of the 8x11pt group, allow a pixel of anti-aliasing.
        self.assertLessEqual(abs((x1 - x0 + 1) - 12), 2)
        self.assertLessEqual(abs((y1 - y0 + 1) - 16), 2)
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

    def test_comparison_sheet_shape(self):
        subprocess.run(
            [sys.executable, str(ROOT / "scripts/gen-tray-compare.py")],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        import struct
        data = (ROOT / "design/tray-weight-compare.png").read_bytes()
        self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
        width, height = struct.unpack(">II", data[16:24])
        self.assertEqual((width, height), (458, 240))

    def test_platform_loading_contract(self):
        source = (ROOT / "apps/desktop/src-tauri/src/tray.rs").read_text()
        self.assertIn('include_bytes!("../icons/tray-template.png")', source)
        self.assertIn('include_bytes!("../icons/tray-windows.png")', source)
        self.assertIn('icon_as_template(cfg!(target_os = "macos"))', source)
        self.assertNotIn("default_window_icon", source)


if __name__ == "__main__":
    unittest.main()
