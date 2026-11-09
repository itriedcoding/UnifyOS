#!/usr/bin/env python3
"""Print kernel modules needed to load the named modules, dependencies first.

Debian's kernel packages ship modules.dep only after ``depmod`` has run, which
needs root and the target kernel. The dependency list is also embedded in every
module's ``.modinfo`` section, so this reads that instead and works unprivileged.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys


def read_depends(path: Path) -> list[str]:
    match = re.search(rb"depends=([^\x00]*)", path.read_bytes())
    if not match:
        return []
    return [name for name in match.group(1).decode("utf-8", "replace").split(",") if name]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--modules-dir", required=True, type=Path)
    parser.add_argument("modules", nargs="+", help="module names, e.g. loop squashfs")
    args = parser.parse_args(argv)

    root: Path = args.modules_dir
    if not root.is_dir():
        print(f"error: {root} is not a directory", file=sys.stderr)
        return 1

    index: dict[str, Path] = {}
    for file in root.rglob("*.ko*"):
        if file.name.endswith((".ko", ".ko.xz", ".ko.zst", ".ko.gz")):
            index.setdefault(file.name.split(".ko")[0].replace("-", "_"), file)

    order: list[str] = []
    seen: set[str] = set()

    def visit(name: str) -> None:
        key = name.replace("-", "_")
        if key in seen:
            return
        path = index.get(key)
        if path is None:
            print(f"warning: module '{name}' not found in {root}", file=sys.stderr)
            return
        seen.add(key)
        for dependency in read_depends(path):
            visit(dependency)
        order.append(path.relative_to(root).as_posix())

    for module in args.modules:
        visit(module)

    if not order:
        print("error: no modules resolved", file=sys.stderr)
        return 1
    print("\n".join(order))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
