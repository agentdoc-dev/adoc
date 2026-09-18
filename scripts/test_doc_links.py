"""Regression checks for the visitor-document link checker; stdlib unittest."""

import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("links", Path(__file__).with_name("check-doc-links.py"))
links = importlib.util.module_from_spec(spec)
spec.loader.exec_module(links)


class Links(unittest.TestCase):
    def test_files_anchors_and_fences(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "other note.md"
            target.write_text("# Hello `world`!\n# Hello world\n")
            page = root / "README.md"
            page.write_text('[one](other%20note.md#hello-world)\n'
                            '[two](other%20note.md#hello-world-1)\n'
                            '````md\n```\n[ignored](missing.md)\n````\n')
            self.assertEqual(links.check([page]), [])
            page.write_text('[bad](other%20note.md#absent)\n[missing](missing.md)\n')
            errors = links.check([page])
            self.assertEqual(len(errors), 2)
            self.assertIn('missing heading', errors[0])
            self.assertIn('missing file', errors[1])


if __name__ == "__main__":
    unittest.main()
