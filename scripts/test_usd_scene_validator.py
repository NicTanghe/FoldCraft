"""Opt-in malformed-input checks for the independent Pixar USD reader.

Run directly with pxr available; this is intentionally outside test_factory.py
and the default CPU suite, since Pixar USD is an optional validation dependency.
"""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECK = ROOT / "scripts" / "check-usd-scene.py"
FIXTURE = ROOT / "tests" / "fixtures" / "usd" / "independent-panel.usda"


class ValidatorTests(unittest.TestCase):
    def run_fixture(self, source):
        with tempfile.TemporaryDirectory(prefix="clothcraft-usd-") as directory:
            path = Path(directory) / "scene.usda"
            path.write_text(source, encoding="utf-8")
            return subprocess.run([sys.executable, str(CHECK), str(path)], capture_output=True, text=True)

    def test_original_deformed_fixture_is_accepted(self):
        result = self.run_fixture(FIXTURE.read_text(encoding="utf-8"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('"triangles": 2', result.stdout)

    def test_invalid_topology_metadata_and_missing_values_are_rejected(self):
        source = FIXTURE.read_text(encoding="utf-8")
        mutations = {
            "bad_index": ("[0, 1, 3, 0, 3, 2]", "[0, 1, 30, 0, 3, 2]"),
            "bad_counts": ("faceVertexCounts = [3, 3]", "faceVertexCounts = [3, 4]"),
            "wrong_scale": ("metersPerUnit = 1", "metersPerUnit = 0.01"),
            "missing_scale": ("    metersPerUnit = 1\n", ""),
            "missing_version": ("        custom int clothcraft:snapshotVersion = 1\n", ""),
            "missing_time": ("        custom double clothcraft:timeSeconds = 0.5\n", ""),
            "missing_rest": ("clothcraft:restUV", "clothcraft:absentUV"),
            "missing_points": ("point3f[] points", "point3f[] missingPoints"),
            "invalid_syntax": ("#usda 1.0", "this is not USDA"),
        }
        for label, (before, after) in mutations.items():
            with self.subTest(label=label):
                self.assertIn(before, source)
                result = self.run_fixture(source.replace(before, after))
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn("USD validation failed:", result.stderr)
                self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    try:
        from pxr import Usd  # noqa: F401
    except ImportError as exc:
        sys.exit(f"Optional Pixar OpenUSD dependency unavailable: {exc}")
    unittest.main()
