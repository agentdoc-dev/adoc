#!/usr/bin/env python3
"""Fail-closed verdict for the required CI status."""

import json
import os
import sys


CORE = ("fmt", "test", "check", "deny")
QUEUE = ("native", "windows")


def gate_result(event_name, needs):
    required = CORE + (QUEUE if event_name == "merge_group" else ())
    failures = {}
    expected = set(CORE + QUEUE)
    for name, job in needs.items():
        if name not in expected:
            failures[name] = f"unexpected:{job.get('result', 'missing')}"
    for name in required:
        result = needs.get(name, {}).get("result")
        if result != "success":
            failures[name] = result if result is not None else "missing"
    if event_name != "merge_group":
        for name in QUEUE:
            result = needs.get(name, {}).get("result")
            if result != "skipped":
                failures[name] = result if result is not None else "missing"
    return failures


def main():
    failures = gate_result(os.environ["EVENT_NAME"], json.loads(os.environ["NEEDS"]))
    for name, result in sorted(failures.items()):
        print(f"{name}: {result}")
    print(f"{len(failures)} failed gate(s)")
    return int(bool(failures))


if __name__ == "__main__":
    sys.exit(main())
