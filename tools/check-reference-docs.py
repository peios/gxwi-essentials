#!/usr/bin/env python3
"""Render manuals and check authored reference prose with the suite's Trail rules."""

import re
import shutil
import subprocess
import sys
import tempfile
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def page(output: Path, name: str, title: str, body: str) -> None:
    """Create a temporary lint page without changing the authored source."""
    (output / name).write_text(f"---\ntitle: {title}\n---\n\n{body}\n", encoding="utf-8")


def manual_text(path: Path) -> str:
    """Check roff rendering and extract prose without treating requests as sentences."""
    result = subprocess.run(
        ["groff", "-Tutf8", "-mandoc", str(path)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        check=True,
    )
    if result.stderr:
        raise ValueError(f"{path}: {result.stderr.strip()}")
    lines = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith(".") or not line.strip():
            continue
        line = line.replace("\\-", "-").replace("\\&", "").replace("\\", "")
        if line.startswith("/"):
            continue
        lines.append(textwrap.fill(line, width=78, break_long_words=False, break_on_hyphens=False))
    return "\n\n".join(lines)


def registry_text(path: Path) -> str:
    """Read record descriptions while preserving their Markdown examples."""
    records = re.split(r"^--- .*$", path.read_text(encoding="utf-8"), flags=re.MULTILINE)[1:]
    if not records:
        raise ValueError(f"{path}: missing registry records")
    bodies = []
    for record in records:
        metadata, separator, body = record.partition("\n\n")
        if not separator or "canonical:" not in metadata or not body.strip():
            raise ValueError(f"{path}: incomplete registry record")
        bodies.append(body)
    return "\n\n".join(bodies)


def main() -> None:
    """Lint repository guides, manual prose and registry descriptions in a disposable tree."""
    trail = sys.argv[1]
    with tempfile.TemporaryDirectory(prefix="gxwi-reference-docs-") as temporary:
        root = Path(temporary)
        shutil.copyfile(ROOT / "docs/learn/trail.toml", root / "trail.toml")
        output = root / "review.product"
        output.mkdir()
        for source in sorted((ROOT / "docs/man").glob("*.1")):
            page(output, source.stem + "-manual.md", "Manual Text", manual_text(source))
        for source in sorted((ROOT / "docs").glob("*.regman")):
            page(output, source.stem + "-registry.md", "Registry Reference", registry_text(source))
        for source in (ROOT / "README.md", ROOT / "docs/building.md"):
            title, _, body = source.read_text(encoding="utf-8").partition("\n")
            page(output, "repo-" + source.name, title.removeprefix("# "), body)
        subprocess.run([trail, "lint", str(root)], check=True)


if __name__ == "__main__":
    main()
