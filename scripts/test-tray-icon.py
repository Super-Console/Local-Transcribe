"""Automatic resource/loading contracts; no GUI or appearance judgement."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "apps/desktop/src-tauri/icons"


class TrayIconTests(unittest.TestCase):
    def test_resources_and_repeatable_generation(self):
        script = ROOT / "scripts/gen-tray-icon.py"
        self.assertTrue(script.is_file(), "generator must exist")
        spec = importlib.util.spec_from_file_location("tray_generator", script)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        names = {"tray-template.png": (22, 0), "tray-template@2x.png": (44, 0),
                 "tray-windows.png": (32, 255)}
        before = {name: (ICONS / name).read_bytes() for name in names}
        subprocess.run([sys.executable, str(script)], check=True)
        for name, (size, color) in names.items():
            data = (ICONS / name).read_bytes()
            self.assertEqual(data, before[name], "generation must be byte-identical")
            width, height, pixels = module.decode_rgba(data)
            self.assertEqual((width, height), (size, size))
            for x, y in [(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1)]:
                self.assertEqual(pixels[(y * size + x) * 4 + 3], 0)
            self.assertTrue(any(pixels[i] == 255 for i in range(3, len(pixels), 4)))
            self.assertTrue(any(pixels[(y * size + size // 2) * 4 + 3] == 255 for y in range(size)))
            for i in range(0, len(pixels), 4):
                if pixels[i + 3]:
                    self.assertEqual(tuple(pixels[i:i + 3]), (color,) * 3)
            self.assertTrue(any(0 < pixels[i] < 255 for i in range(3, len(pixels), 4)), "antialiased edge")

    def test_platform_loading_contract(self):
        source = (ROOT / "apps/desktop/src-tauri/src/tray.rs").read_text()
        self.assertNotIn("default_window_icon", source)
        self.assertIn('include_bytes!("../icons/tray-template.png")', source)
        self.assertIn('include_bytes!("../icons/tray-windows.png")', source)
        self.assertIn('icon_as_template(cfg!(target_os = "macos"))', source)
        self.assertIn('"image-png"', (ROOT / "apps/desktop/src-tauri/Cargo.toml").read_text())


if __name__ == "__main__":
    unittest.main()
