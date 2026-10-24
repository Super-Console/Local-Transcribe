"""Tray resources for the three-bar design; structural checks only.

Taskbook "菜单栏图标重做" §4: the @2x midline scan must find exactly three
runs of 3px (±1) separated by 6px (±1) gaps, and opaque pixels must stay under
12% of the canvas. No appearance judgement.
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


def runs(pixels, size, y):
    segments, start = [], None
    for x in range(size):
        alpha = pixels[(y * size + x) * 4 + 3]
        if alpha > 0 and start is None:
            start = x
        elif alpha == 0 and start is not None:
            segments.append((start, x - 1))
            start = None
    if start is not None:
        segments.append((start, size - 1))
    return segments


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

    def test_midline_has_three_thin_bars(self):
        width, _, pixels = self.read("tray-template@2x.png")
        segments = runs(pixels, width, 22)
        self.assertEqual(len(segments), 3, f"expected three bars, got {segments}")
        widths = [end - start + 1 for start, end in segments]
        gaps = [segments[index + 1][0] - segments[index][1] - 1 for index in range(2)]
        for value in widths:
            self.assertLessEqual(abs(value - 3), 1, f"bar width {widths}")
        for value in gaps:
            self.assertLessEqual(abs(value - 6), 1, f"gap {gaps}")

    def test_opaque_area_stays_under_the_cap(self):
        for name, size in [("tray-template.png", 22), ("tray-template@2x.png", 44), ("tray-windows.png", 32)]:
            width, _, pixels = self.read(name)
            opaque = sum(1 for index in range(3, len(pixels), 4) if pixels[index] == 255)
            self.assertLessEqual(opaque / (width * width), 0.12, name)

    def test_macos_template_is_black(self):
        for name in ("tray-template.png", "tray-template@2x.png"):
            width, _, pixels = self.read(name)
            for index in range(0, len(pixels), 4):
                if pixels[index + 3]:
                    self.assertEqual(tuple(pixels[index:index + 3]), (0, 0, 0), name)

    def test_windows_uses_the_same_shape_in_white(self):
        width, _, pixels = self.read("tray-windows.png")
        segments = runs(pixels, width, 16)
        self.assertEqual(len(segments), 3, f"expected three bars, got {segments}")
        for start, end in segments:
            self.assertLessEqual(abs((end - start + 1) - 3), 1)
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
