"""Menu-bar assets reuse the exact app-logo glyph at a smaller uniform scale."""
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


def components(pixels, size, threshold=1):
    alpha = [pixels[index * 4 + 3] if pixels[index * 4 + 3] >= threshold else 0
             for index in range(size * size)]
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


def bounding_box(pixels, size, threshold=128):
    points = [(index % size, index // size) for index in range(size * size)
              if pixels[index * 4 + 3] >= threshold]
    xs = [x for x, _ in points]
    ys = [y for _, y in points]
    return min(xs), max(xs), min(ys), max(ys)


class TrayIconTests(unittest.TestCase):
    def setUp(self):
        self.module = load("gen-tray-icon")
        self.logo = load("gen-quote-icon")

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
        self.assertEqual(len(components(pixels, width, threshold=64)), 2)

    def test_bounding_box_and_centring(self):
        width, _, pixels = self.read("tray-template@2x.png")
        x0, x1, y0, y1 = bounding_box(pixels, width, threshold=128)
        box_width, box_height = x1 - x0 + 1, y1 - y0 + 1
        self.assertGreaterEqual(box_width, 23)
        self.assertLessEqual(box_width, 27)
        self.assertGreaterEqual(box_height, 18)
        self.assertLessEqual(box_height, 21)
        self.assertLessEqual(abs(x0 - (width - 1 - x1)), 1)
        self.assertLessEqual(abs(y0 - (width - 1 - y1)), 1)

    def test_source_logo_proportions_are_preserved(self):
        width, _, pixels = self.read("tray-template@2x.png")
        x0, x1, y0, y1 = bounding_box(pixels, width, threshold=128)
        source_glyph = self.logo.glyph_from_source(self.logo.SOURCE)
        source_ratio = source_glyph[5] / source_glyph[6]
        actual_ratio = (x1 - x0 + 1) / (y1 - y0 + 1)
        self.assertAlmostEqual(actual_ratio, source_ratio, delta=0.08)

    def test_alpha_mask_matches_the_app_logo_glyph(self):
        width, _, pixels = self.read("tray-template@2x.png")
        source_glyph = self.logo.glyph_from_source(self.logo.SOURCE)
        source_pixels, source_width, source_height, gx, gy, glyph_width, glyph_height = source_glyph
        target_width = self.module.SCALE_WIDTHS["B"]
        target_height = target_width * glyph_height / glyph_width
        left = (self.module.CANVAS - target_width) / 2
        top = (self.module.CANVAS - target_height) / 2
        expected, actual = set(), set()

        for y in range(width):
            py = (y + 0.5) * self.module.CANVAS / width
            for x in range(width):
                index = (y * width + x) * 4 + 3
                if pixels[index] >= 128:
                    actual.add((x, y))
                px = (x + 0.5) * self.module.CANVAS / width
                if left <= px < left + target_width and top <= py < top + target_height:
                    sx = gx + (px - left) * glyph_width / target_width - 0.5
                    sy = gy + (py - top) * glyph_height / target_height - 0.5
                    alpha = self.logo.sample_bilinear(
                        source_pixels, source_width, source_height, sx, sy
                    )[3]
                    if alpha >= 128:
                        expected.add((x, y))

        intersection = len(expected & actual)
        union = len(expected | actual)
        self.assertGreaterEqual(intersection / union, 0.95)

    def test_macos_template_is_black(self):
        for name in ("tray-template.png", "tray-template@2x.png"):
            width, _, pixels = self.read(name)
            for index in range(0, len(pixels), 4):
                if pixels[index + 3]:
                    self.assertEqual(tuple(pixels[index:index + 3]), (0, 0, 0), name)

    def test_windows_same_shape_in_white(self):
        width, _, pixels = self.read("tray-windows.png")
        self.assertEqual(len(components(pixels, width, threshold=128)), 2)
        x0, x1, y0, y1 = bounding_box(pixels, width, threshold=128)
        self.assertLessEqual(abs((x1 - x0 + 1) - 18), 2)
        self.assertLessEqual(abs((y1 - y0 + 1) - 14), 2)
        source_glyph = self.logo.glyph_from_source(self.logo.SOURCE)
        source_ratio = source_glyph[5] / source_glyph[6]
        self.assertLessEqual(abs((x1 - x0 + 1) / (y1 - y0 + 1) - source_ratio), 0.12)
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
        self.assertIn('include_bytes!("../icons/tray-template@2x.png")', source)
        self.assertIn('include_bytes!("../icons/tray-windows.png")', source)
        self.assertIn('icon_as_template(cfg!(target_os = "macos"))', source)
        self.assertNotIn("default_window_icon", source)


if __name__ == "__main__":
    unittest.main()
