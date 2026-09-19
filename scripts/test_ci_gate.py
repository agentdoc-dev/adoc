import importlib.util
import pathlib
import unittest


SPEC = importlib.util.spec_from_file_location(
    "check_ci_gate", pathlib.Path(__file__).with_name("check-ci-gate.py")
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
gate_result = MODULE.gate_result


CORE = {name: {"result": "success"} for name in ("fmt", "test", "check", "deny")}


class CiGateTests(unittest.TestCase):
    def test_pr_allows_intentionally_skipped_native_jobs(self):
        needs = {
            **CORE,
            "native": {"result": "skipped"},
            "windows": {"result": "skipped"},
        }
        self.assertEqual(gate_result("pull_request", needs), {})
        self.assertTrue(gate_result("pull_request", CORE))

    def test_core_failure_skip_and_missing_fail(self):
        for needs in (
            {**CORE, "test": {"result": "failure"}},
            {**CORE, "check": {"result": "skipped"}},
            {name: value for name, value in CORE.items() if name != "deny"},
        ):
            with self.subTest(needs=needs):
                self.assertTrue(gate_result("pull_request", needs))

    def test_queue_requires_native_and_windows_success(self):
        queue = {**CORE, "native": {"result": "success"}, "windows": {"result": "success"}}
        self.assertEqual(gate_result("merge_group", queue), {})
        for name, result in (("native", "failure"), ("native", "skipped"), ("windows", "cancelled")):
            with self.subTest(name=name, result=result):
                needs = {**queue, name: {"result": result}}
                self.assertTrue(gate_result("merge_group", needs))
        missing = {name: value for name, value in queue.items() if name != "windows"}
        self.assertTrue(gate_result("merge_group", missing))

    def test_unknown_result_fails_closed(self):
        self.assertTrue(gate_result("pull_request", {**CORE, "fmt": {"result": "neutral"}}))

    def test_unknown_dependency_fails_closed(self):
        needs = {**CORE, "native": {"result": "skipped"}, "windows": {"result": "skipped"}}
        self.assertEqual(gate_result("pull_request", {**needs, "security": {"result": "failure"}}), {"security": "unexpected:failure"})


if __name__ == "__main__":
    unittest.main()
