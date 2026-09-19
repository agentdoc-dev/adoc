#!/usr/bin/env python3
"""Focused checks for the performance qualification harness."""

import hashlib
import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


MODULE = Path(__file__).with_name("benchmark.py")
SPEC = importlib.util.spec_from_file_location("benchmark", MODULE)
benchmark = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = benchmark
SPEC.loader.exec_module(benchmark)


class CorpusAndSummaryTest(unittest.TestCase):
    def test_rejects_unbounded_configuration_before_corpus_write(self):
        with self.assertRaisesRegex(RuntimeError, "at most 3"):
            benchmark.validate_configuration([1, 2, 3, 4], 5)
        with self.assertRaisesRegex(RuntimeError, "between 1 and 10000"):
            benchmark.validate_configuration([10_001], 5)
        with self.assertRaisesRegex(RuntimeError, "between 5 and 30"):
            benchmark.validate_configuration([1], 31)
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "between 1 and 10000"):
                benchmark.write_corpus(Path(directory), 10_001)

    def test_corpus_is_deterministic_and_has_refund_target(self):
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            first_root = Path(first)
            second_root = Path(second)
            benchmark.write_corpus(first_root, 10)
            benchmark.write_corpus(second_root, 10)
            first_bytes = (first_root / "docs/index.adoc").read_bytes()
            second_bytes = (second_root / "docs/index.adoc").read_bytes()
        self.assertEqual(first_bytes, second_bytes)
        self.assertIn(b"::claim billing.refund-window", first_bytes)
        self.assertIn(b"refund", first_bytes)
        self.assertEqual(benchmark.sha256_bytes(first_bytes), hashlib.sha256(first_bytes).hexdigest())

    def test_summary_keeps_raw_samples_and_uses_no_percentile_claim(self):
        samples = [
            {"scenario": "check", "elapsed_seconds": 0.3, "correct": True},
            {"scenario": "check", "elapsed_seconds": 0.1, "correct": True},
        ]
        summary = benchmark.summarize(samples)
        self.assertEqual(summary["check"]["sample_count"], 2)
        self.assertEqual(summary["check"]["minimum_seconds"], 0.1)
        self.assertEqual(summary["check"]["median_seconds"], 0.2)
        self.assertNotIn("percentile", str(summary).lower())

    def test_command_record_rejects_failed_command_and_invalid_result(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "exit 7"):
                benchmark.command_record([sys.executable, "-c", "raise SystemExit(7)"], directory,
                                         "failed", lambda _: None)
            with self.assertRaisesRegex(RuntimeError, "correctness failed"):
                benchmark.command_record([sys.executable, "-c", "print('bad')"], directory,
                                         "invalid", lambda _: (_ for _ in ()).throw(ValueError("bad result")))

    def test_embedding_provision_uses_child_only_cache_environment(self):
        original_cache_dir = benchmark.os.environ.get("FASTEMBED_CACHE_DIR")
        with tempfile.TemporaryDirectory() as directory, patch.dict(
                benchmark.os.environ, {"FASTEMBED_CACHE_DIR": "parent-cache"}, clear=False):
            root = Path(directory)
            cache_dir = root / "task-cache"
            observed = []

            def fake_command(command, cwd, scenario, correctness, timeout=benchmark.TIMEOUT_SECONDS, env=None):
                observed.append((command, cwd, scenario, env))
                return {"scenario": scenario, "correct": True}

            with patch.object(benchmark, "command_record", side_effect=fake_command):
                setup = benchmark.provision_embedding_model(Path("/bin/adoc"), root, cache_dir)
            corpus = (root / "docs/index.adoc").read_text(encoding="utf-8")

        self.assertEqual(observed[0][0], ["/bin/adoc", "build"])
        self.assertEqual(observed[0][2], "embedding_model_provision")
        self.assertEqual(observed[0][3]["FASTEMBED_CACHE_DIR"], str(cache_dir))
        self.assertEqual(benchmark.os.environ.get("FASTEMBED_CACHE_DIR"), original_cache_dir)
        self.assertEqual(setup["corpus_size"], 1)
        self.assertIn("billing.refund-window", corpus)


if __name__ == "__main__":
    unittest.main()
