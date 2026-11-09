"""Installation planning for UnifyOS.

A :class:`InstallPlan` is a validated, side-effect-free description of an
installation. Every command the installer runs is derived from the plan here,
so the dangerous parts stay reviewable and unit-testable without a disk.

Layout produced (GPT):

    UEFI: 1 = EFI system partition (512 MiB, FAT32, ESP)
          2 = swap (optional)
          3 = root (ext4)
    BIOS: 1 = BIOS boot partition (1 MiB, no filesystem)
          2 = swap (optional)
          3 = root (ext4)
"""

from __future__ import annotations

from dataclasses import dataclass
import re
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

MIB = 1024 * 1024
GIB = 1024 * MIB

MIN_DISK_MIB = 6 * 1024
ESP_MIB = 512
BIOS_BOOT_MIB = 1
ALIGNMENT_RESERVE_MIB = 32
MIN_ROOT_MIB = 4 * 1024
MAX_SWAP_MIB = 4096

HOSTNAME_RE = re.compile(r"^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$")
USERNAME_RE = re.compile(r"^[a-z_][a-z0-9_-]{0,31}$")
LOCALE_RE = re.compile(r"^[a-z]{2}_[A-Z]{2}\.UTF-8$")
KEYMAP_RE = re.compile(r"^[a-zA-Z0-9-]+$")


class PlanError(ValueError):
    """The requested installation is invalid or unsafe."""


@dataclass(frozen=True)
class InstallPlan:
    disk: str
    firmware: str
    hostname: str
    timezone: str
    locale: str
    keymap: str
    username: str
    full_name: str
    swap_mib: int
    root_mib: int
    disk_mib: int

    def __post_init__(self) -> None:
        if not self.disk.startswith("/dev/"):
            raise PlanError(f"disk must be a block device path, got {self.disk!r}")
        if self.firmware not in ("uefi", "bios"):
            raise PlanError("firmware must be 'uefi' or 'bios'")
        if not HOSTNAME_RE.match(self.hostname):
            raise PlanError(f"invalid hostname {self.hostname!r}")
        if not USERNAME_RE.match(self.username):
            raise PlanError(f"invalid username {self.username!r}")
        if not LOCALE_RE.match(self.locale):
            raise PlanError(f"invalid locale {self.locale!r} (expected e.g. en_US.UTF-8)")
        if not KEYMAP_RE.match(self.keymap):
            raise PlanError(f"invalid keyboard layout {self.keymap!r}")
        try:
            ZoneInfo(self.timezone)
        except (ZoneInfoNotFoundError, ValueError) as error:
            raise PlanError(f"unknown timezone {self.timezone!r}") from error
        if self.swap_mib < 0:
            raise PlanError("swap size cannot be negative")
        if self.disk_mib < MIN_DISK_MIB:
            raise PlanError(
                f"disk is too small: {self.disk_mib} MiB, need at least {MIN_DISK_MIB} MiB"
            )
        if self.root_mib < MIN_ROOT_MIB:
            raise PlanError(
                f"root filesystem is too small: {self.root_mib} MiB, need at least {MIN_ROOT_MIB} MiB"
            )
        used = self.esp_mib + self.boot_mib + self.swap_mib + self.root_mib
        if used > self.disk_mib:
            raise PlanError(
                f"partitions need {used} MiB but the disk has {self.disk_mib} MiB"
            )

    @property
    def esp_mib(self) -> int:
        return ESP_MIB if self.firmware == "uefi" else 0

    @property
    def boot_mib(self) -> int:
        return 0 if self.firmware == "uefi" else BIOS_BOOT_MIB

    @property
    def total_used_mib(self) -> int:
        return self.esp_mib + self.boot_mib + self.swap_mib + self.root_mib


def swap_size_mib(ram_mib: int) -> int:
    """Swap policy: 2x RAM on small machines, capped, none on very large ones."""
    if ram_mib >= 16 * 1024:
        return 0
    if ram_mib <= 2048:
        return ram_mib * 2
    return min(ram_mib, MAX_SWAP_MIB)


def auto_plan(
    *,
    disk: str,
    firmware: str,
    disk_mib: int,
    ram_mib: int,
    hostname: str,
    timezone: str,
    locale: str,
    keymap: str,
    username: str,
    full_name: str,
    swap_mib: int | None = None,
) -> InstallPlan:
    """Build a plan that uses the whole disk."""
    reserved = (ESP_MIB if firmware == "uefi" else BIOS_BOOT_MIB) + ALIGNMENT_RESERVE_MIB
    swap = swap_size_mib(ram_mib) if swap_mib is None else swap_mib
    root = disk_mib - reserved - swap
    if root < MIN_ROOT_MIB:
        raise PlanError(
            f"not enough space for a root filesystem after reserved areas "
            f"({disk_mib} MiB disk, {reserved + swap} MiB reserved)"
        )
    return InstallPlan(
        disk=disk,
        firmware=firmware,
        hostname=hostname,
        timezone=timezone,
        locale=locale,
        keymap=keymap,
        username=username,
        full_name=full_name,
        swap_mib=swap,
        root_mib=root,
        disk_mib=disk_mib,
    )


def partition_device(disk: str, index: int) -> str:
    """Device path for a partition: /dev/sda1 but /dev/nvme0n1p1."""
    if disk and disk[-1].isdigit():
        return f"{disk}p{index}"
    return f"{disk}{index}"


@dataclass(frozen=True)
class PartitionLayout:
    esp: str | None
    bios_boot: str | None
    swap: str | None
    root: str


def partition_layout(plan: InstallPlan) -> PartitionLayout:
    index = 1
    esp = bios_boot = swap = None
    if plan.firmware == "uefi":
        esp = partition_device(plan.disk, index)
        index += 1
    else:
        bios_boot = partition_device(plan.disk, index)
        index += 1
    if plan.swap_mib > 0:
        swap = partition_device(plan.disk, index)
        index += 1
    root = partition_device(plan.disk, index)
    return PartitionLayout(esp=esp, bios_boot=bios_boot, swap=swap, root=root)


def partition_commands(plan: InstallPlan) -> list[list[str]]:
    """sgdisk invocations that create the GPT layout."""
    # --zap-all is the data-destroying step; callers must have confirmed the
    # target explicitly (see disk.check_install_target).
    commands: list[list[str]] = [["sgdisk", "--zap-all", plan.disk]]
    index = 1
    if plan.firmware == "uefi":
        commands.append(
            [
                "sgdisk",
                f"--new={index}:0:+{ESP_MIB}M",
                f"--typecode={index}:ef00",
                f"--change-name={index}:UNIFY-ESP",
                plan.disk,
            ]
        )
        index += 1
    else:
        commands.append(
            [
                "sgdisk",
                f"--new={index}:0:+{BIOS_BOOT_MIB}M",
                f"--typecode={index}:ef02",
                f"--change-name={index}:UNIFY-BOOT",
                plan.disk,
            ]
        )
        index += 1
    if plan.swap_mib > 0:
        commands.append(
            [
                "sgdisk",
                f"--new={index}:0:+{plan.swap_mib}M",
                f"--typecode={index}:8200",
                f"--change-name={index}:UNIFY-SWAP",
                plan.disk,
            ]
        )
        index += 1
    commands.append(
        [
            "sgdisk",
            f"--new={index}:0:0",
            f"--typecode={index}:8300",
            f"--change-name={index}:UNIFY-ROOT",
            plan.disk,
        ]
    )
    return commands


def format_commands(plan: InstallPlan) -> list[list[str]]:
    layout = partition_layout(plan)
    commands: list[list[str]] = []
    if layout.esp:
        commands.append(["mkfs.fat", "-F", "32", "-n", "UNIFY-ESP", layout.esp])
    if layout.swap:
        commands.append(["mkswap", "-L", "unify-swap", layout.swap])
    commands.append(["mkfs.ext4", "-L", "unify-root", "-m", "1", "-F", layout.root])
    return commands


def mount_commands(plan: InstallPlan, target: str) -> list[list[str]]:
    layout = partition_layout(plan)
    commands: list[list[str]] = [["mount", layout.root, target]]
    if layout.esp:
        commands.append(["mount", "--mkdir", layout.esp, f"{target}/boot/efi"])
    if layout.swap:
        commands.append(["swapon", layout.swap])
    return commands


def fstab(plan: InstallPlan, uuids: dict[str, str]) -> str:
    """fstab content using filesystem UUIDs (no device-name fragility)."""
    layout = partition_layout(plan)
    lines = [
        "# /etc/fstab — generated by the UnifyOS installer",
        f"UUID={uuids['root']}\t/\text4\terrors=remount-ro\t0\t1",
    ]
    if layout.esp:
        lines.append(
            f"UUID={uuids['esp']}\t/boot/efi\tvfat\tumask=0077\t0\t1"
        )
    if layout.swap and "swap" in uuids:
        lines.append(f"UUID={uuids['swap']}\tnone\tswap\tsw\t0\t0")
    return "\n".join(lines) + "\n"
