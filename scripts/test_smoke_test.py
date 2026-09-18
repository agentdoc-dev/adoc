"""Regression checks for the portable smoke-test harness."""

import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = Path(__file__).with_name("smoke-test.py")
SPEC = importlib.util.spec_from_file_location("smoke_test", MODULE_PATH)
smoke_test = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(smoke_test)


class ExecutablePathTest(unittest.TestCase):
    def test_uses_platform_executable_suffix(self):
        binaries = Path("installed binaries")
        self.assertEqual(smoke_test.executable_path(binaries, "adoc", "posix"), binaries / "adoc")
        self.assertEqual(smoke_test.executable_path(binaries, "adoc-mcp", "posix"), binaries / "adoc-mcp")
        self.assertEqual(smoke_test.executable_path(binaries, "adoc", "nt"), binaries / "adoc.exe")
        self.assertEqual(smoke_test.executable_path(binaries, "adoc-mcp", "nt"), binaries / "adoc-mcp.exe")


if __name__ == "__main__":
    unittest.main()
