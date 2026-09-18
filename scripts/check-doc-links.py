#!/usr/bin/env python3
"""Check relative file links and heading anchors in visitor-facing Markdown."""

from collections import Counter
import re
from pathlib import Path
import sys
import unicodedata
from urllib.parse import unquote, urlsplit


def prose(text):
    fence = None
    for line in text.splitlines():
        match = re.match(r"^\s*(`{3,}|~{3,})", line)
        if match:
            marker = match[1]
            if fence is None:
                fence = marker
            elif marker[0] == fence[0] and len(marker) >= len(fence):
                fence = None
            continue
        if fence is None:
            yield line


def anchors(text):
    result, counts = set(), Counter()
    for line in prose(text):
        result.update(re.findall(r'<a\s+(?:id|name)=["\']([^"\']+)', line))
        heading = re.match(r"^#{1,6}\s+(.+?)\s*#*\s*$", line)
        if not heading:
            continue
        title = re.sub(r"\[([^]]+)\]\([^)]*\)", r"\1", heading[1])
        title = re.sub(r"<[^>]*>", "", title).lower()
        slug = "".join(c for c in title if c in "-_ " or unicodedata.category(c)[0] in "LN")
        slug = slug.replace(" ", "-")
        count = counts[slug]
        counts[slug] += 1
        result.add(f"{slug}-{count}" if count else slug)
    return result


def check(files):
    errors = []
    for path in files:
        for line in prose(path.read_text()):
            for target in re.findall(r"\]\(([^\s)]+)(?:\s+\"[^\"]*\")?\)", line):
                url = urlsplit(target)
                if url.scheme or url.netloc:
                    continue
                destination = path.parent / unquote(url.path) if url.path else path
                if not destination.exists():
                    errors.append(f"{path}: missing file {target}")
                elif url.fragment and destination.suffix == ".md":
                    if unquote(url.fragment) not in anchors(destination.read_text()):
                        errors.append(f"{path}: missing heading {target}")
    return errors


def main():
    root = Path(__file__).resolve().parents[1]
    files = [root / name for name in ("README.md", "CONTRIBUTING.md", "SECURITY.md",
             "docs/product/README.md", "docs/roadmap/v10/README.md")]
    files += sorted((root / "docs/guides").glob("*.md"))
    files += sorted((root / "docs/reference").glob("*.md"))
    errors = check(files)
    for error in errors:
        print(error, file=sys.stderr)
    print(f"Checked relative links and heading anchors in {len(files)} entry-point documents")
    return bool(errors)


if __name__ == "__main__":
    sys.exit(main())
