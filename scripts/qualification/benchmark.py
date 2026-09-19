#!/usr/bin/env python3
"""Reproducible local performance evidence for an installed AgentDoc release."""

import argparse
import hashlib
import json
import os
import platform
import re
import queue
import threading
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path


TIMEOUT_SECONDS = 180
MCP_TIMEOUT_SECONDS = 30
TARGET_ID = "billing.refund-window"
TARGET_QUERY = "refund"
MAX_CORPUS_SIZE = 10_000
MAX_CORPUS_SIZES = 3
MAX_CORPUS_TOTAL = 30_000
MIN_SAMPLES = 5
MAX_SAMPLES = 30
_TIME_SUPPORTED = None


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def sha256_file(path):
    return sha256_bytes(path.read_bytes())


def executable_path(directory, name):
    return directory / f"{name}{'.exe' if os.name == 'nt' else ''}"


def write_corpus(root, size):
    """Write a bounded deterministic corpus with a known citable refund claim."""
    require(1 <= size <= MAX_CORPUS_SIZE,
            f"corpus size must be between 1 and {MAX_CORPUS_SIZE}")
    docs = root / "docs"
    docs.mkdir(parents=True, exist_ok=True)
    claims = [
        "# Qualification corpus @doc(qualification.performance)\n\n",
        "::claim billing.refund-window\nstatus: draft\nowner: billing\n--\n"
        "Customers can request a refund within 30 days of purchase.\n::\n\n",
    ]
    for number in range(1, size):
        claims.append(
            f"::claim qualification.synthetic-{number:05d}\nstatus: draft\nowner: qualification\n--\n"
            f"Synthetic qualification record {number:05d}; stable corpus content.\n::\n\n"
        )
    (docs / "index.adoc").write_text("".join(claims), encoding="utf-8", newline="\n")


def summarize(samples):
    grouped = {}
    for sample in samples:
        grouped.setdefault(sample["scenario"], []).append(sample["elapsed_seconds"])
    return {
        scenario: {
            "sample_count": len(values),
            "minimum_seconds": min(values),
            "median_seconds": statistics.median(values),
            "maximum_seconds": max(values),
        }
        for scenario, values in sorted(grouped.items())
    }


def command_record(command, cwd, scenario, correctness, timeout=TIMEOUT_SECONDS, env=None):
    """Run a bounded CLI command and retain raw timing, result, and RSS observation."""
    global _TIME_SUPPORTED
    wrapped = list(command)
    rss_unit = "unavailable"
    if _TIME_SUPPORTED is None:
        _TIME_SUPPORTED = Path("/usr/bin/time").is_file() and subprocess.run(
            ["/usr/bin/time", "-l", "true"], capture_output=True, timeout=5
        ).returncode == 0 if sys.platform == "darwin" else Path("/usr/bin/time").is_file()
    if _TIME_SUPPORTED and sys.platform == "darwin":
        wrapped = ["/usr/bin/time", "-l", *command]
        rss_unit = "bytes"
    elif _TIME_SUPPORTED and sys.platform.startswith("linux"):
        wrapped = ["/usr/bin/time", "-v", *command]
        rss_unit = "KiB"
    started = time.perf_counter()
    try:
        result = subprocess.run(wrapped, cwd=cwd, env=env, capture_output=True, text=True,
                                encoding="utf-8", timeout=timeout)
    except subprocess.TimeoutExpired as error:
        raise RuntimeError(f"{scenario} timed out after {timeout}s: {' '.join(command)}") from error
    elapsed = time.perf_counter() - started
    combined = result.stdout + result.stderr
    require(result.returncode == 0,
            f"{scenario} exit {result.returncode}: {' '.join(command)}\n{combined[-2000:]}")
    try:
        correctness(result.stdout)
        correct = True
    except Exception as error:
        raise RuntimeError(f"{scenario} correctness failed: {error}\n{combined[-2000:]}") from error
    rss = None
    if rss_unit == "bytes":
        match = re.search(r"(\d+)\s+maximum resident set size", result.stderr)
        rss = int(match.group(1)) if match else None
    elif rss_unit == "KiB":
        match = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", result.stderr)
        rss = int(match.group(1)) if match else None
    return {
        "scenario": scenario, "command": command, "elapsed_seconds": elapsed,
        "timeout_seconds": timeout, "exit_code": result.returncode, "correct": correct,
        "stdout_sha256": sha256_bytes(result.stdout.encode()),
        "stderr_sha256": sha256_bytes(result.stderr.encode()), "peak_rss": rss,
        "peak_rss_unit": rss_unit if rss is not None else "unavailable",
        "peak_rss_note": None if rss is not None else "native child RSS unavailable for this command",
    }


def citation(payload):
    records = payload["records"]
    matches = [record for record in records if record.get("id") == TARGET_ID]
    require(matches, f"{TARGET_ID} missing")
    source = matches[0]["source"]
    require(source["path"] == "docs/index.adoc" and source["line"] > 0,
            "refund citation is incomplete")


def json_citation(stdout):
    citation(json.loads(stdout))


def mcp_request(server, messages, identifier, method, params):
    server.stdin.write(json.dumps({"jsonrpc": "2.0", "id": identifier, "method": method,
                                   "params": params}) + "\n")
    server.stdin.flush()
    deadline = time.monotonic() + MCP_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        try:
            line = messages.get(timeout=max(0, deadline - time.monotonic()))
        except queue.Empty as error:
            raise RuntimeError(f"MCP {method} timed out after {MCP_TIMEOUT_SECONDS}s") from error
        require(line, f"MCP exited during {method}")
        response = json.loads(line)
        if response.get("id") == identifier:
            require("error" not in response, f"MCP {method} error: {response}")
            return response["result"]
    raise RuntimeError(f"MCP {method} timed out after {MCP_TIMEOUT_SECONDS}s")


def mcp_samples(binary, root, sample_count, records, env=None):
    for number in range(sample_count):
        started = time.perf_counter()
        errors = tempfile.TemporaryFile()
        server = subprocess.Popen([str(binary)], cwd=root, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=errors, text=True, encoding="utf-8", bufsize=1)
        messages = queue.Queue()
        def read_responses():
            for line in server.stdout:
                messages.put(line)
            messages.put(None)
        reader = threading.Thread(target=read_responses, daemon=True)
        reader.start()
        try:
            mcp_request(server, messages, 1, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                                    "clientInfo": {"name": "adoc-benchmark", "version": "1"}})
            server.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
            server.stdin.flush()
            tools = mcp_request(server, messages, 2, "tools/list", {})
            require({"adoc_project_status", "adoc_why"} <= {item["name"] for item in tools["tools"]},
                    "MCP retrieval tools missing")
            records.append({"scenario": "mcp_startup", "command": [str(binary)],
                            "interaction": "initialize, notifications/initialized, tools/list",
                            "result_sha256": sha256_bytes(json.dumps(tools, sort_keys=True).encode()),
                            "elapsed_seconds": time.perf_counter() - started,
                            "timeout_seconds": MCP_TIMEOUT_SECONDS, "correct": True,
                            "peak_rss": None, "peak_rss_unit": "unavailable",
                            "peak_rss_note": "persistent MCP child RSS is not measured by stdlib"})
            if number != sample_count - 1:
                continue
            # Warm the same server once, then measure repeated requests in that session.
            for repetition in range(sample_count + 1):
                for scenario, name, arguments, validator in (
                    ("mcp_status", "adoc_project_status", {"project_root": str(root)},
                     lambda value: require(not value.get("isError") and value["structuredContent"]["readiness"]["retrieval"], "MCP retrieval not ready")),
                    ("mcp_why", "adoc_why", {"project_root": str(root), "object_id": TARGET_ID},
                     lambda value: citation(value["structuredContent"])),
                ):
                    started = time.perf_counter()
                    result = mcp_request(server, messages, 3 if scenario == "mcp_status" else 4, "tools/call",
                                         {"name": name, "arguments": arguments})
                    validator(result)
                    if repetition == 0:
                        continue
                    records.append({"scenario": scenario, "interaction": f"tools/call {name}",
                                    "result_sha256": sha256_bytes(json.dumps(result, sort_keys=True).encode()),
                                    "elapsed_seconds": time.perf_counter() - started,
                                    "timeout_seconds": MCP_TIMEOUT_SECONDS, "correct": True,
                                    "peak_rss": None, "peak_rss_unit": "unavailable",
                                    "peak_rss_note": "persistent MCP child RSS is not measured by stdlib"})
        finally:
            if server.poll() is None:
                server.terminate()
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)
            server.stdin.close()
            reader.join(timeout=5)
            server.stdout.close()
            errors.close()


def metadata(binary_dir, binary_source_revision):
    source = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True,
                            encoding="utf-8", timeout=10)
    return {"platform": platform.platform(), "cpu": platform.processor() or "unreported",
            "python": sys.version, "harness_revision": source.stdout.strip() if source.returncode == 0 else "unavailable",
            "binary_source_revision": binary_source_revision,
            "machine": platform.machine(), "logical_cpu_count": os.cpu_count(),
            "binaries": {name: sha256_file(executable_path(binary_dir, name)) for name in ("adoc", "adoc-mcp")}}


def validate_configuration(sizes, samples):
    require(sizes, "--sizes must not be empty")
    require(len(sizes) <= MAX_CORPUS_SIZES,
            f"--sizes accepts at most {MAX_CORPUS_SIZES} values")
    require(all(1 <= value <= MAX_CORPUS_SIZE for value in sizes),
            f"--sizes values must be between 1 and {MAX_CORPUS_SIZE}")
    require(sum(sizes) <= MAX_CORPUS_TOTAL,
            f"--sizes total must not exceed {MAX_CORPUS_TOTAL}")
    require(MIN_SAMPLES <= samples <= MAX_SAMPLES,
            f"--samples must be between {MIN_SAMPLES} and {MAX_SAMPLES}")


def child_environment(cache_dir):
    """Return a child-only environment using the harness-owned model cache."""
    env = os.environ.copy()
    env["FASTEMBED_CACHE_DIR"] = str(cache_dir)
    return env


def cache_file_digests(cache_dir):
    if not cache_dir.is_dir():
        return {}
    return {
        str(path.relative_to(cache_dir)): sha256_file(path)
        for path in sorted(cache_dir.rglob("*")) if path.is_file()
    }


def provision_embedding_model(adoc, root, cache_dir):
    """Initialize and embed one object outside all measured corpus runs."""
    shutil.rmtree(root / "dist", ignore_errors=True)
    shutil.rmtree(root / "docs", ignore_errors=True)
    write_corpus(root, 1)
    source = root / "docs/index.adoc"
    env = child_environment(cache_dir)
    record = command_record([str(adoc), "build"], root, "embedding_model_provision",
                            lambda _: None, env=env)
    return {"corpus_size": 1, "source_sha256": sha256_file(source), "samples": [record],
            "model_cache_file_sha256": cache_file_digests(cache_dir),
            "note": "Setup includes model initialization and one embedding; it is not model-download timing."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-revision", default="unverified", help="source identity used to build these binaries")
    parser.add_argument("--sizes", default="100,1000,10000")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--embeddings", action="store_true")
    args = parser.parse_args()
    sizes = [int(value) for value in args.sizes.split(",")]
    validate_configuration(sizes, args.samples)
    binaries = args.bin_dir.resolve()
    for name in ("adoc", "adoc-mcp"):
        require(executable_path(binaries, name).is_file(), f"missing {name} in {binaries}")
    evidence = {"schema_version": "adoc.performance.v1", "metadata": metadata(binaries, args.source_revision),
                "configuration": {"sizes": sizes, "samples": args.samples, "embeddings": args.embeddings,
                                  "timeout_seconds": TIMEOUT_SECONDS}, "runs": [],
                "limitations": ["No percentile, SLA, cross-host comparison, or cache-cold claim is made.",
                                "Embedding setup includes model initialization and one embedding; model download is not timed separately.",
                                "Synthetic corpus generation requires no network; embedding provisioning may require model access."]}
    with tempfile.TemporaryDirectory(prefix="adoc-benchmark-") as temporary:
        root = Path(temporary)
        model_cache = root / "fastembed-cache"
        env = child_environment(model_cache)
        adoc = executable_path(binaries, "adoc")
        init = command_record([str(adoc), "init"], root, "setup_init", lambda _: None, env=env)
        if args.embeddings:
            evidence["embedding_setup"] = provision_embedding_model(adoc, root, model_cache)
        for size in sizes:
            shutil.rmtree(root / "dist", ignore_errors=True)
            shutil.rmtree(root / "docs", ignore_errors=True)
            write_corpus(root, size)
            source = root / "docs/index.adoc"
            run = {"size": size, "source_sha256": sha256_file(source), "samples": [init] if size == sizes[0] else []}
            for _ in range(args.samples):
                run["samples"].append(command_record([str(adoc), "check"], root, "check", lambda _: None, env=env))
                run["samples"].append(command_record([str(adoc), "build", "--no-embeddings"], root, "build_no_embeddings", lambda _: None, env=env))
                run["samples"].append(command_record([str(adoc), "search", TARGET_QUERY, "--lexical", "--format", "json"], root, "search_lexical", json_citation, env=env))
            run["embedding_model"] = "not_requested"
            if args.embeddings:
                run["samples"].append(command_record([str(adoc), "build"], root, "build_embeddings_first_computed", lambda _: None, env=env))
                search = json.loads((root / "dist/docs.search.json").read_text(encoding="utf-8"))
                require(search["schema_version"] == "adoc.search.v2", "unexpected search schema")
                require(search["model"]["provider"] == "fastembed", "FastEmbed provider schema missing")
                run["embedding_model"] = search["model"]
                for _ in range(args.samples):
                    run["samples"].append(command_record([str(adoc), "build"], root, "build_embeddings_cached", lambda _: None, env=env))
                    run["samples"].append(command_record([str(adoc), "search", TARGET_QUERY, "--semantic", "--format", "json"], root, "search_semantic", json_citation, env=env))
                    run["samples"].append(command_record([str(adoc), "search", TARGET_QUERY, "--format", "json"], root, "search_hybrid", json_citation, env=env))
            mcp_samples(executable_path(binaries, "adoc-mcp"), root, args.samples, run["samples"], env=env)
            run["summary"] = summarize(run["samples"])
            evidence["runs"].append(run)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
