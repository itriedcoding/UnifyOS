"""Command-line installer front end.

Usage examples::

    unify-installer --list-disks
    unify-installer --disk /dev/sda --hostname unify-pc --timezone Europe/Berlin \
                    --username alice --full-name "Alice" --dry-run
    unify-installer --disk /dev/sda --hostname unify-pc --yes --password-stdin

The graphical front end is not implemented yet; see documentation/limitations.md.
"""

from __future__ import annotations

import argparse
import getpass
import json
from pathlib import Path
import sys

from . import plan as planning
from .disk import SafetyError, check_install_target, list_block_devices
from .install import InstallError, Installer


def detect_firmware() -> str:
    return "uefi" if Path("/sys/firmware/efi").exists() else "bios"


def detect_ram_mib() -> int:
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemTotal:"):
                return int(line.split()[1]) // 1024
    except (OSError, ValueError, IndexError):
        pass
    return 2048


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="unify-installer", description="Install UnifyOS to a disk"
    )
    parser.add_argument("--list-disks", action="store_true", help="show candidate disks")
    parser.add_argument("--disk", help="target disk, e.g. /dev/sda")
    parser.add_argument("--firmware", choices=("uefi", "bios"))
    parser.add_argument("--hostname", default="unifyos")
    parser.add_argument("--timezone", default="UTC")
    parser.add_argument("--locale", default="en_US.UTF-8")
    parser.add_argument("--keymap", default="us")
    parser.add_argument("--username", default="unify")
    parser.add_argument("--full-name", default="UnifyOS User")
    parser.add_argument("--swap-mib", type=int, help="override automatic swap size")
    parser.add_argument("--root-mib", type=int, help="manual root partition size")
    parser.add_argument("--target", default="/mnt/unify")
    parser.add_argument("--dry-run", action="store_true", help="print commands only")
    parser.add_argument("--yes", action="store_true", help="skip the confirmation prompt")
    parser.add_argument("--allow-removable", action="store_true")
    parser.add_argument("--allow-system-disk", action="store_true")
    parser.add_argument("--password-stdin", action="store_true")
    parser.add_argument("--json", action="store_true", help="machine-readable result")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    devices = list_block_devices()

    if args.list_disks:
        if args.json:
            print(json.dumps([device.__dict__ for device in devices], indent=2))
        else:
            for device in devices:
                kind = "removable" if device.removable else ("hdd" if device.rotational else "ssd")
                print(f"{device.path:<16} {device.human_size:>10}  {kind:<10} {device.model}")
        return 0

    if not args.disk:
        print("error: --disk is required (use --list-disks to see candidates)", file=sys.stderr)
        return 2

    device = next((candidate for candidate in devices if candidate.path == args.disk), None)
    if device is None:
        print(f"error: {args.disk} is not a discovered disk", file=sys.stderr)
        return 2

    if not args.dry_run:
        try:
            check_install_target(
                device.path,
                size_bytes=device.size_bytes,
                removable=device.removable,
                mountinfo=Path("/proc/self/mountinfo").read_text(),
                allow_removable=args.allow_removable,
                allow_system_disk=args.allow_system_disk,
            )
        except SafetyError as error:
            print(f"safety check failed: {error}", file=sys.stderr)
            return 3

    firmware = args.firmware or detect_firmware()
    disk_mib = device.size_bytes // planning.MIB

    try:
        if args.root_mib:
            plan = planning.InstallPlan(
                disk=device.path,
                firmware=firmware,
                hostname=args.hostname,
                timezone=args.timezone,
                locale=args.locale,
                keymap=args.keymap,
                username=args.username,
                full_name=args.full_name,
                swap_mib=args.swap_mib or 0,
                root_mib=args.root_mib,
                disk_mib=disk_mib,
            )
        else:
            plan = planning.auto_plan(
                disk=device.path,
                firmware=firmware,
                disk_mib=disk_mib,
                ram_mib=detect_ram_mib(),
                hostname=args.hostname,
                timezone=args.timezone,
                locale=args.locale,
                keymap=args.keymap,
                username=args.username,
                full_name=args.full_name,
                swap_mib=args.swap_mib,
            )
    except planning.PlanError as error:
        print(f"invalid installation plan: {error}", file=sys.stderr)
        return 2

    print(
        f"Target   {plan.disk} ({device.human_size}, {plan.firmware})\n"
        f"Layout   ESP {plan.esp_mib} MiB, swap {plan.swap_mib} MiB, root {plan.root_mib} MiB\n"
        f"Identity {plan.username}@{plan.hostname}, {plan.timezone}, {plan.locale}, {plan.keymap}"
    )
    if plan.disk == "/dev/sda" and device.removable:
        print("warning: the target looks like the live medium", file=sys.stderr)

    if not args.yes and not args.dry_run:
        answer = input(f"Type the disk name ({device.name}) to erase it: ").strip()
        if answer != device.name:
            print("aborted; nothing was written", file=sys.stderr)
            return 1

    password: str | None = None
    if args.password_stdin:
        password = sys.stdin.readline().rstrip("\n")
    elif not args.dry_run and sys.stdin.isatty():
        first = getpass.getpass(f"Password for {plan.username}: ")
        second = getpass.getpass("Repeat password: ")
        if first != second:
            print("passwords do not match", file=sys.stderr)
            return 2
        if len(first) < 8:
            print("password must be at least 8 characters", file=sys.stderr)
            return 2
        password = first

    installer = Installer(plan, target=args.target, dry_run=args.dry_run)
    try:
        commands = installer.run(password=password)
    except (InstallError, SafetyError) as error:
        print(f"installation failed: {error}", file=sys.stderr)
        print(f"full log: {installer.log_path}", file=sys.stderr)
        return 4

    if args.json:
        print(json.dumps({"plan": plan.__dict__, "commands": commands}, indent=2, default=str))
    elif args.dry_run:
        print(f"{len(commands)} commands planned; nothing was executed")
    else:
        print("Installation complete. Remove the medium and reboot.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
