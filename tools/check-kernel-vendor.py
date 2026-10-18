#!/usr/bin/env python3
"""Verify that the selected kernel source still matches its pinned export."""

import hashlib
import json
from pathlib import Path


def main() -> None:
    """Reject any modified or missing upstream file recorded in the source manifest."""
    root = Path("vendor/kernel")
    checksums = json.loads((root / "source-sha256.json").read_text(encoding="utf-8"))
    for relative, expected in checksums.items():
        source = root / relative
        if hashlib.sha256(source.read_bytes()).hexdigest() != expected:
            raise SystemExit(f"kernel source differs from pinned export: {relative}")


if __name__ == "__main__":
    main()
