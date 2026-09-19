#!/usr/bin/env python3
"""Normalize `wast-runner --format json` output into a committed baseline.

Reads runner JSON on stdin and writes a stable, checkout-independent baseline on
stdout:

* file paths are reduced to basenames (so the baseline does not depend on the
  absolute checkout directory),
* per-directive `diagnostics` (volatile, wording-dependent) are dropped,
* the top-level `summary` (which has no bearing on the per-case gate) is dropped,
* files and directives are sorted for a deterministic diff.

Usage:
    cargo run --features native --bin wast-runner -- tests/wast_fixtures \
        --format json | scripts/normalize-baseline.py \
        > tests/baseline/wast_fixtures_baseline.json
"""

import json
import os
import sys


def main() -> None:
    data = json.load(sys.stdin)
    for f in data.get("files", []):
        f["file"] = os.path.basename(f["file"])
        for directive in f.get("directives", []):
            directive.pop("diagnostics", None)
    data.pop("summary", None)
    data["files"].sort(key=lambda x: x["file"])
    json.dump(data, sys.stdout, indent=2, sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
