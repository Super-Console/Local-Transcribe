"""Structural checks for the quote-mark icon set; no appearance judgement."""
import importlib.util
import struct
from pathlib import Path
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


def rgba(path):
    return load("gen-tray-icon").decode_rgba(path.read_bytes())


class QuoteIconTests(unittest.TestCase):
    def test_master_is_a_cream_square_with_a_coral_glyph(self):
        width, height, pixels = rgba(ICONS / "icon.png")
        self.assertEqual((width, height), (1024, 1024))
        for index in (0, 1023, 1023 * 1024, 1024 * 1024 - 1):
            self.assertEqual(pixels[index * 4 + 3], 0, "corners are transparent")
        center = (512 * 1024 + 512) * 4
        self.assertEqual(pixels[center + 3], 255)
        r, g, b = pixels[center : center + 3]
        self.assertGreater(r, 240)
        self.assertGreater(g, 235)
        self.assertGreater(b, 225)
        coral = sum(
            1
            for i in range(0, len(pixels), 4)
            if pixels[i + 3] == 255 and pixels[i] - pixels[i + 1] > 60
        )
        self.assertGreater(coral, 20000, "the quotation glyph must be present")

    def test_every_bundle_size_exists_and_keeps_transparent_corners(self):
        plan = {
            "32x32.png": 32,
            "64x64.png": 64,
            "128x128.png": 128,
            "128x128@2x.png": 256,
        }
        for name, size in plan.items():
            width, height, pixels = rgba(ICONS / name)
            self.assertEqual((width, height), (size, size), name)
            for index in (0, size - 1, size * (size - 1), size * size - 1):
                self.assertEqual(pixels[index * 4 + 3], 0, name)

    def test_icns_and_ico_contain_the_expected_containers(self):
        self.assertTrue((ICONS / "icon.icns").read_bytes().startswith(b"icns"))
        data = (ICONS / "icon.ico").read_bytes()
        reserved, kind, count = struct.unpack("<HHH", data[:6])
        self.assertEqual((reserved, kind, count), (0, 1, 6))
        self.assertGreater(len(data), 1000)

    def test_windows_store_logos_follow_their_names(self):
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
            width, height, _ = rgba(ICONS / name)
            self.assertEqual((width, height), (size, size), name)

    def test_favicon_matches_the_app_icon(self):
        self.assertEqual(
            (ROOT / "apps/desktop/static/favicon.png").read_bytes(),
            (ICONS / "icon.png").read_bytes(),
        )


if __name__ == "__main__":
    unittest.main()
