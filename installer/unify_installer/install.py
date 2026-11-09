"""Executing an :class:`InstallPlan` against a real disk.

Every step is logged (console and ``/var/log/unify-install.log``) and every
external command is recorded, so a failed installation can be diagnosed from
the log alone. ``dry_run=True`` records the command sequence without executing
anything, which is what the test-suite exercises.
"""

from __future__ import annotations

from datetime import datetime
import os
from pathlib import Path
import shutil
import subprocess

from . import plan as planning
from .disk import system_disks

DEFAULT_TARGET = "/mnt/unify"
DEFAULT_LOG = "/var/log/unify-install.log"
COPY_EXCLUDES = (
    "proc",
    "sys",
    "dev",
    "run",
    "tmp",
    "mnt",
    "media",
    "lost+found",
    "boot/efi",
    "var/cache/apt/archives",
)
REQUIRED_TOOLS = ("sgdisk", "mkfs.ext4", "mkfs.fat", "rsync", "blkid", "mount", "chroot")


class InstallError(RuntimeError):
    """An installation step failed."""


class Installer:
    def __init__(
        self,
        plan: planning.InstallPlan,
        *,
        target: str = DEFAULT_TARGET,
        dry_run: bool = False,
        log_path: str = DEFAULT_LOG,
        source_root: str = "/",
        verify_environment: bool = True,
    ) -> None:
        self.plan = plan
        self.target = target
        self.dry_run = dry_run
        self.log_path = log_path
        self.source_root = source_root
        self.verify_environment = verify_environment and not dry_run
        self.commands: list[list[str]] = []

    # -- helpers ---------------------------------------------------------
    def log(self, message: str) -> None:
        line = f"[{datetime.now():%H:%M:%S}] {message}"
        print(line, flush=True)
        try:
            Path(self.log_path).parent.mkdir(parents=True, exist_ok=True)
            with open(self.log_path, "a", encoding="utf-8") as handle:
                handle.write(line + "\n")
        except OSError:
            pass

    def _run(
        self,
        argv: list[str],
        *,
        check: bool = True,
        stdin_text: str | None = None,
    ) -> subprocess.CompletedProcess:
        self.commands.append(list(argv))
        self.log("$ " + " ".join(argv))
        if self.dry_run:
            return subprocess.CompletedProcess(argv, 0, "", "")
        result = subprocess.run(argv, capture_output=True, text=True, input=stdin_text)
        for stream in (result.stdout, result.stderr):
            if stream and stream.strip():
                self.log(stream.strip())
        if check and result.returncode != 0:
            raise InstallError(
                f"command failed with status {result.returncode}: {' '.join(argv)}"
            )
        return result

    # -- steps -----------------------------------------------------------
    def run(self, *, password: str | None = None) -> list[list[str]]:
        self.log(
            f"UnifyOS installation to {self.plan.disk} "
            f"({self.plan.firmware}, {self.plan.root_mib} MiB root, "
            f"{self.plan.swap_mib} MiB swap)"
        )
        self._preflight()
        self._partition()
        self._format()
        self._mount()
        self._copy_system()
        self._write_fstab()
        self._configure_system(password)
        self._install_bootloader()
        self._finish()
        self.log("installation finished successfully")
        return self.commands

    def _preflight(self) -> None:
        if not self.verify_environment:
            return
        for tool in REQUIRED_TOOLS:
            if shutil.which(tool) is None:
                raise InstallError(f"required tool is missing: {tool}")
        if os.geteuid() != 0:
            raise InstallError("the installer must run as root (boot the live medium)")
        mountinfo = Path("/proc/self/mountinfo").read_text()
        disk_name = self.plan.disk.rsplit("/", 1)[-1]
        if disk_name in system_disks(mountinfo):
            raise InstallError(
                f"{self.plan.disk} backs the running system; refusing to install"
            )
        if not Path(self.plan.disk).exists():
            raise InstallError(f"{self.plan.disk} does not exist")

    def _partition(self) -> None:
        for command in planning.partition_commands(self.plan):
            self._run(command)
        self._run(["partprobe", self.plan.disk], check=False)

    def _format(self) -> None:
        for command in planning.format_commands(self.plan):
            self._run(command)

    def _mount(self) -> None:
        if not self.dry_run:
            Path(self.target).mkdir(parents=True, exist_ok=True)
        for command in planning.mount_commands(self.plan, self.target):
            self._run(command)

    def _copy_system(self) -> None:
        source = self.source_root.rstrip("/") + "/"
        destination = self.target.rstrip("/") + "/"
        argv = ["rsync", "-aHAX", "--one-file-system", "--numeric-ids"]
        argv += [f"--exclude=/{exclude}" for exclude in COPY_EXCLUDES]
        argv += [source, destination]
        self._run(argv)

    def _write_fstab(self) -> None:
        layout = planning.partition_layout(self.plan)
        if self.dry_run:
            uuids = {
                "root": "<uuid-of-root>",
                "esp": "<uuid-of-esp>",
                "swap": "<uuid-of-swap>",
            }
        else:
            uuids = {}

            def uuid_of(device: str) -> str:
                result = self._run(["blkid", "-s", "UUID", "-o", "value", device])
                return result.stdout.strip()

            uuids["root"] = uuid_of(layout.root)
            if layout.esp:
                uuids["esp"] = uuid_of(layout.esp)
            if layout.swap:
                uuids["swap"] = uuid_of(layout.swap)

        content = planning.fstab(self.plan, uuids)
        self.log("generated /etc/fstab:\n" + content)
        if self.dry_run:
            return
        fstab_path = Path(self.target) / "etc/fstab"
        fstab_path.parent.mkdir(parents=True, exist_ok=True)
        fstab_path.write_text(content, encoding="utf-8")

    def _configure_system(self, password: str | None) -> None:
        script = f"""#!/bin/bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
echo "{self.plan.hostname}" > /etc/hostname
ln -sf /usr/share/zoneinfo/{self.plan.timezone} /etc/localtime
echo "{self.plan.locale}" > /etc/locale.gen
locale-gen
echo "LANG={self.plan.locale}" > /etc/default/locale
echo "KEYMAP={self.plan.keymap}" > /etc/vconsole.conf
id -u {self.plan.username} >/dev/null 2>&1 || \\
    useradd -m -s /bin/bash -c "{self.plan.full_name}" \\
    -G sudo,video,audio,input,netdev,bluetooth,plugdev {self.plan.username}
systemctl enable NetworkManager.service
systemctl enable unify-sysmond.service
systemctl enable fstrim.timer 2>/dev/null || true
update-initramfs -u -k all
"""
        if self.dry_run:
            self.log(
                "would run in-target configuration for "
                f"{self.plan.hostname} / {self.plan.username}"
            )
            if password:
                self.commands.append(["chroot", self.target, "chpasswd"])
            return
        script_path = Path(self.target) / "tmp" / "unify-configure.sh"
        script_path.parent.mkdir(parents=True, exist_ok=True)
        script_path.write_text(script, encoding="utf-8")
        script_path.chmod(0o755)
        self._run(["chroot", self.target, "/bin/bash", "/tmp/unify-configure.sh"])
        script_path.unlink(missing_ok=True)
        if password:
            self._run(
                ["chroot", self.target, "chpasswd"],
                stdin_text=f"{self.plan.username}:{password}\n",
            )
        else:
            self.log("no password supplied; account is locked until one is set")

    def _install_bootloader(self) -> None:
        if self.plan.firmware == "uefi":
            self._run(
                [
                    "chroot",
                    self.target,
                    "grub-install",
                    "--target=x86_64-efi",
                    "--efi-directory=/boot/efi",
                    "--bootloader-id=UnifyOS",
                    "--recheck",
                ]
            )
        else:
            self._run(
                [
                    "chroot",
                    self.target,
                    "grub-install",
                    "--target=i386-pc",
                    "--recheck",
                    self.plan.disk,
                ]
            )
        self._run(["chroot", self.target, "update-grub"])

    def _finish(self) -> None:
        layout = planning.partition_layout(self.plan)
        if layout.swap:
            self._run(["swapoff", layout.swap], check=False)
        self._run(["umount", "-R", self.target], check=False)
        self.log("all filesystems unmounted")
