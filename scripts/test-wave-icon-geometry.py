"""Check the user's taller waveform update without making visual judgements."""
import importlib.util
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WaveGeometryTests(unittest.TestCase):
    def test_app_taller_geometry_stays_centered(self):
        icon = load("gen-app-icon")
        self.assertEqual([shape[3] for shape in icon.SHAPES[:3]], [168, 384, 240])
        for x, y, width, height, radius in icon.SHAPES[:3]:
            self.assertEqual(y + height / 2, 512)
            self.assertEqual((width, radius), (62, 31))
        self.assertEqual(icon.SHAPES[3], (566, 481, 226, 62, 31))

    def test_mac_tray_wave_height_and_windows_unchanged_geometry(self):
        icon = load("gen-tray-icon")
        for size, color, expected in [(22, 0, [8, 17, 11]), (44, 0, [16, 34, 22]),
                                       (32, 255, None)]:
            _, _, pixels = icon.decode_rgba(icon.render(size, color))
            if expected:
                heights = []
                for x in [round(4.5 * size / 22 - .5), round(10.5 * size / 22 - .5), round(16.5 * size / 22 - .5)]:
                    heights.append(sum(pixels[(y * size + x) * 4 + 3] / 255 for y in range(size)))
                for measured, wanted in zip(heights, expected):
                    self.assertAlmostEqual(measured, wanted, delta=.35)


if __name__ == "__main__":
    unittest.main()
