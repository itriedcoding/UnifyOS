"""Block-device discovery and install-target safety checks.

Every check here exists to stop the installer from destroying the disk it is
running from, or a removable medium the user did not mean to target.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import re

MIN_TARGET_BYTES = 6 * 1024**3


class SafetyError(RuntimeError):
    """The requested install target is unsafe."""


@dataclass(frozen=True)
class BlockDevice:
    name: str
    path: str
    size_bytes: int
    model: str
    removable: bool
    rotational: bool
    partitions: tuple[str, ...]

    @property
    def human_size(self) -> str:
        return f"{self.size_bytes / 1000**3:.1f} GB"


def _read(path: Path) -> str | None:
    try:
        return path.read_text().strip()
    except OSError:
        return None


def list_block_devices(sys_block: Path = Path("/sys/block")) -> list[BlockDevice]:
    """Enumerate real disks (loop, ram, zram, device-mapper and md are skipped)."""
    devices: list[BlockDevice] = []
    if not sys_block.is_dir():
        return devices
    for entry in sorted(sys_block.iterdir()):
        name = entry.name
        if name.startswith(("loop", "ram", "zram", "dm-", "md", "sr")):
            continue
        sectors = int(_read(entry / "size") or 0)
        size_bytes = sectors * 512
        if size_bytes == 0:
            continue
        partitions = tuple(
            sorted(
                child.name
                for child in entry.iterdir()
                if child.name.startswith(name) and (child / "partition").exists()
            )
        )
        devices.append(
            BlockDevice(
                name=name,
                path=f"/dev/{name}",
                size_bytes=size_bytes,
                model=(_read(entry / "device/model") or "").strip(),
                removable=_read(entry / "removable") == "1",
                rotational=_read(entry / "queue/rotational") == "1",
                partitions=partitions,
            )
        )
    return devices


def _base_device(source: str) -> str | None:
    if not source.startswith("/dev/"):
        return None
    name = source[len("/dev/") :]
    if name.startswith(("nvme", "mmcblk")):
        return re.sub(r"p\d+$", "", name)
    match = re.match(r"^([a-zA-Z]+)(\d+)$", name)
    return match.group(1) if match else name


def system_disks(mountinfo: str) -> set[str]:
    """Disks that currently back the running system (never valid install targets)."""
    disks: set[str] = set()
    for line in mountinfo.splitlines():
        fields = line.split()
        if len(fields) < 5:
            continue
        try:
            separator = fields.index("-")
        except ValueError:
            continue
        if separator + 2 >= len(fields):
            continue
        base = _base_device(fields[separator + 2])
        if base:
            disks.add(base)
    return disks


def check_install_target(
    device: str,
    *,
    size_bytes: int,
    removable: bool,
    mountinfo: str,
    min_bytes: int = MIN_TARGET_BYTES,
    allow_removable: bool = False,
    allow_system_disk: bool = False,
) -> None:
    """Refuse obviously destructive targets before anything is written."""
    if not device.startswith("/dev/"):
        raise SafetyError(f"target must be a block device path, got {device!r}")
    if size_bytes < min_bytes:
        raise SafetyError(
            f"target {device} is too small ({size_bytes / 1000**3:.1f} GB); "
            f"at least {min_bytes / 1000**3:.0f} GB is required"
        )
    if removable and not allow_removable:
        raise SafetyError(
            f"{device} is removable; pass --allow-removable if you really mean to "
            f"install to that medium"
        )
    disk_name = device.rsplit("/", 1)[-1]
    base = re.sub(r"p?\d+$", "", disk_name) if not disk_name.startswith("nvme") else re.sub(r"p\d+$", "", disk_name)
    running = system_disks(mountinfo)
    if base in running and not allow_system_disk:
        raise SafetyError(
            f"{device} holds the currently running system; refusing to overwrite it. "
            f"Boot the live medium and choose another disk"
        )
