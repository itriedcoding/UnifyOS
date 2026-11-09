#!/usr/bin/env python3
"""Generate the GRUB index files that Debian's binary packages do not ship.

``grub-mkimage`` and ``grub-mkstandalone`` read ``moddep.lst``, ``command.lst``,
``fs.lst``, ``partmap.lst``, ``parttool.lst``, ``video.lst``, ``terminal.lst``
and ``crypto.lst`` from the module directory. Debian generates them at package
build time and ships only for the installed architecture, so an extracted
toolchain cannot build images without recreating them.

Every list is derivable from the ``.modinfo`` section embedded in each ``.mod``:
it holds NUL-terminated ``key=value`` strings such as ``depends=part_msdos`` or
``fs=ext2``. This script reads those and writes the lists in GRUB's format.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys

KEY_RE = re.compile(rb"(depends|commands|fs|partmap|parttool|video|terminal|crypto)=([\x20-\x7e]*)")

# list file -> modinfo key
LISTS = {
    "moddep.lst": "depends",
    "command.lst": "commands",
    "fs.lst": "fs",
    "partmap.lst": "partmap",
    "parttool.lst": "parttool",
    "video.lst": "video",
    "terminal.lst": "terminal",
    "crypto.lst": "crypto",
}


def values_for(data: bytes, key: str) -> list[str]:
    for match in KEY_RE.finditer(data):
        if match.group(1).decode() == key:
            raw = match.group(2).decode("utf-8", "replace").strip()
            # GRUB separates names with spaces; some builds use commas.
            return [item for item in re.split(r"[,\s]+", raw) if item]
    return []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", required=True, type=Path,
                        help="GRUB module directory, e.g. /usr/lib/grub/x86_64-efi")
    parser.add_argument("--also-parent", action="store_true",
                        help="also write the lists into the parent directory "
                             "(grub-mkstandalone looks there)")
    args = parser.parse_args(argv)

    directory: Path = args.directory
    modules = sorted(directory.glob("*.mod"))
    if not modules:
        print(f"error: no .mod files in {directory}", file=sys.stderr)
        return 1

    lists: dict[str, list[str]] = {name: [] for name in LISTS}
    for module in modules:
        data = module.read_bytes()
        name = module.stem
        for list_name, key in LISTS.items():
            items = values_for(data, key)
            if key == "depends":
                lists[list_name].append(f"{name}: {' '.join(items)}")
            else:
                for item in items:
                    lists[list_name].append(f"{item}: {name}")

    targets = [directory]
    if args.also_parent:
        targets.append(directory.parent)

    for target in targets:
        for list_name, lines in lists.items():
            (target / list_name).write_text("\n".join(lines) + "\n")

    print(f"{directory.name}: {len(modules)} modules, "
          f"{len(lists['command.lst'])} commands, {len(lists['fs.lst'])} filesystems")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
