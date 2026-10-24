"""Tray resource contracts for the quote-mark icon; no appearance judgement.

The canonical generator is scripts/gen-quote-icon.py (it also produces the app
icon, .icns/.ico and Windows logos). Running the old waveform generator would
overwrite these files, so this test only re-runs the quote generator.
"""
import hashlib
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


class TrayIconTests(unittest.TestCase):
    def test_generation_is_repeatable(self):
        names = ["tray-template.png", "tray-template@2x.png", "tray-windows.png", "icon.png"]
        before = {name: (ICONS / name).read_bytes() for name in names}
        subprocess.run(
            [sys.executable, str(ROOT / "scripts/gen-quote-icon.py")],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        for name in names:
            after = (ICONS / name).read_bytes()
            self.assertEqual(
                hashlib.sha256(after).hexdigest(),
                hashlib.sha256(before[name]).hexdigest(),
                f"{name} must regenerate byte-identically",
            )

    def test_macos_template_is_black_with_alpha(self):
        module = load("gen-tray-icon")
        for name, size in [("tray-template.png", 22), ("tray-template@2x.png", 44)]:
            width, height, pixels = module.decode_rgba((ICONS / name).read_bytes())
            self.assertEqual((width, height), (size, size), name)
            for x, y in [(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1)]:
                self.assertEqual(pixels[(y * size + x) * 4 + 3], 0)
            self.assertTrue(any(pixels[i] == 255 for i in range(3, len(pixels), 4)))
            for i in range(0, len(pixels), 4):
                if pixels[i + 3]:
                    self.assertEqual(tuple(pixels[i : i + 3]), (0, 0, 0), name)
            self.assertTrue(
                any(0 < pixels[i] < 255 for i in range(3, len(pixels), 4)),
                f"{name} keeps antialiased edges",
            )

    def test_windows_tray_uses_the_colour_app_icon(self):
        self.assertEqual(
            (ICONS / "tray-windows.png").read_bytes(),
            (ICONS / "32x32.png").read_bytes(),
        )

    def test_platform_loading_contract(self):
        source = (ROOT / "apps/desktop/src-tauri/src/tray.rs").read_text()
        self.assertNotIn("default_window_icon", source)
        self.assertIn('include_bytes!("../icons/tray-template.png")', source)
        self.assertIn('include_bytes!("../icons/tray-windows.png")', source)
        self.assertIn('icon_as_template(cfg!(target_os = "macos"))', source)
        self.assertIn('"image-png"', (ROOT / "apps/desktop/src-tauri/Cargo.toml").read_text())


if __name__ == "__main__":
    unittest.main()
