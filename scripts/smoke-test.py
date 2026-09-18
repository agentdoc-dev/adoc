#!/usr/bin/env python3
"""Exercise the documented CLI/MCP journey in a disposable project (stdlib only)."""

import argparse
import json
from pathlib import Path
import queue
import subprocess
import tempfile
import threading


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, default=Path("target/debug"))
    parser.add_argument("--embeddings", action="store_true", help="also exercise real model retrieval")
    args = parser.parse_args()
    binaries = args.bin_dir.resolve()
    for name in ("adoc", "adoc-mcp"):
        require((binaries / name).is_file(), f"Build or unpack {name} in {binaries} first")

    with tempfile.TemporaryDirectory(prefix="adoc-smoke-") as directory:
        root = Path(directory)

        def cli(*arguments, expected=0):
            result = subprocess.run(
                [str(binaries / "adoc"), *arguments], cwd=root,
                capture_output=True, text=True, timeout=180,
            )
            require(result.returncode == expected,
                    f"adoc {' '.join(arguments)}: exit {result.returncode}\n{result.stdout}{result.stderr}")
            return result.stdout + result.stderr

        cli("init")
        source = root / "docs/index.adoc"
        source.write_text((Path(__file__).resolve().parents[1] / "examples/quickstart/refunds.adoc").read_text())
        original = source.read_bytes()
        require("init.already_exists" in cli("init", expected=1), "init must refuse existing targets")
        require(source.read_bytes() == original, "init changed existing source")
        require("0 errors, 0 warnings" in cli("check"), "example did not validate cleanly")
        cli("build", "--no-embeddings")
        require((root / "dist/docs.html").is_file(), "HTML missing")
        graph = json.loads((root / "dist/docs.graph.json").read_text())
        require(graph["schema_version"] == "adoc.graph.v6", "unexpected graph version")

        def citation(result):
            records = result["records"]
            matches = [r for r in records if r.get("id") == "billing.refund-window"]
            require(matches, "refund claim missing from retrieval")
            require(matches[0]["source"]["path"] == "docs/index.adoc", "source citation missing")
            require(matches[0]["source"]["line"] > 0, "source line missing")

        citation(json.loads(cli("why", "billing.refund-window", "--format", "json")))
        citation(json.loads(cli("search", "refund", "--lexical", "--format", "json")))
        if args.embeddings:
            cli("build")
            model = json.loads((root / "dist/docs.search.json").read_text())["model"]
            require(model["provider"] == "fastembed", "real model was not used")
            citation(json.loads(cli("search", "refund", "--semantic", "--format", "json")))

        source.write_text(original.decode() + "\nSee [[missing.object]].\n")
        require("ref.broken" in cli("check", expected=1), "invalid source was not rejected")
        source.write_bytes(original)
        print("CLI: init, validation, build, citations, search and safe failures passed", flush=True)

        with tempfile.TemporaryFile(mode="w+") as errors:
            server = subprocess.Popen(
                [str(binaries / "adoc-mcp")], cwd=root, stdin=subprocess.PIPE,
                stdout=subprocess.PIPE, stderr=errors, text=True, bufsize=1,
            )
            messages = queue.Queue()

            def reader():
                for line in server.stdout:
                    messages.put(line)
                messages.put(None)

            thread = threading.Thread(target=reader, daemon=True)
            thread.start()

            def send(message):
                server.stdin.write(json.dumps(message) + "\n")
                server.stdin.flush()

            def request(identifier, method, params):
                send({"jsonrpc": "2.0", "id": identifier, "method": method, "params": params})
                while True:
                    line = messages.get(timeout=30)
                    require(line is not None, "MCP server exited before responding")
                    response = json.loads(line)
                    if response.get("id") == identifier:
                        require("error" not in response, f"MCP error: {response}")
                        return response["result"]

            try:
                request(1, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                        "clientInfo": {"name": "adoc-smoke", "version": "1"}})
                send({"jsonrpc": "2.0", "method": "notifications/initialized"})
                listed = request(2, "tools/list", {})
                require("adoc_why" in {t["name"] for t in listed["tools"]}, "retrieval tool missing")
                status = request(3, "tools/call", {"name": "adoc_project_status",
                                 "arguments": {"project_root": str(root)}})
                require(not status.get("isError"), f"MCP project status failed: {status}")
                state = status["structuredContent"]
                require(state["readiness"]["retrieval"], "MCP retrieval not ready")
                require(not state["readiness"]["patch_apply_enabled"], "patch apply enabled by default")
                result = request(4, "tools/call", {"name": "adoc_why", "arguments": {
                    "project_root": str(root), "object_id": "billing.refund-window"}})
                require(not result.get("isError"), f"MCP retrieval failed: {result}")
                citation(result["structuredContent"])
                print("MCP: handshake, tools, readiness, citation and default write boundary passed")
            finally:
                server.terminate()
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait()
                server.stdin.close()
                thread.join(timeout=5)
                server.stdout.close()


if __name__ == "__main__":
    main()
