"""Unit tests for installation planning, disk safety and dry-run execution."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from unify_installer import plan as planning
from unify_installer.disk import SafetyError, check_install_target, system_disks
from unify_installer.install import Installer

BASE = dict(
    disk="/dev/sda",
    firmware="uefi",
    disk_mib=64 * 1024,
    ram_mib=4096,
    hostname="unifyos",
    timezone="UTC",
    locale="en_US.UTF-8",
    keymap="us",
    username="unify",
    full_name="UnifyOS User",
)

MOUNTINFO = (
    "36 35 98:0 / / rw,relatime - ext4 /dev/nvme0n1p2 rw\n"
    "37 36 8:1 / /boot/efi rw,relatime - vfat /dev/sda1 rw\n"
    "38 36 0:40 / /run rw,nosuid - tmpfs tmpfs rw\n"
)


class SwapPolicyTests(unittest.TestCase):
    def test_small_machines_get_double_ram(self):
        self.assertEqual(planning.swap_size_mib(1024), 2048)
        self.assertEqual(planning.swap_size_mib(2048), 4096)

    def test_medium_machines_are_capped(self):
        self.assertEqual(planning.swap_size_mib(8192), planning.MAX_SWAP_MIB)

    def test_large_machines_get_no_swap(self):
        self.assertEqual(planning.swap_size_mib(32768), 0)


class PlanTests(unittest.TestCase):
    def test_auto_plan_fits_on_disk(self):
        plan = planning.auto_plan(**BASE)
        self.assertEqual(plan.esp_mib, 512)
        self.assertEqual(plan.boot_mib, 0)
        self.assertEqual(plan.total_used_mib <= plan.disk_mib, True)
        self.assertGreaterEqual(plan.root_mib, planning.MIN_ROOT_MIB)

    def test_bios_plan_uses_bios_boot_partition(self):
        plan = planning.auto_plan(**{**BASE, "firmware": "bios"})
        self.assertEqual(plan.esp_mib, 0)
        self.assertEqual(plan.boot_mib, 1)
        layout = planning.partition_layout(plan)
        self.assertIsNone(layout.esp)
        self.assertEqual(layout.bios_boot, "/dev/sda1")

    def test_disk_too_small_is_rejected(self):
        with self.assertRaises(planning.PlanError):
            planning.auto_plan(**{**BASE, "disk_mib": 4096})

    def test_invalid_identity_is_rejected(self):
        cases = {
            "hostname": "Bad Host",
            "username": "9nope",
            "locale": "english",
            "timezone": "Mars/Olympus",
            "keymap": "us;rm -rf",
        }
        for field, value in cases.items():
            with self.subTest(field=field):
                with self.assertRaises(planning.PlanError):
                    planning.auto_plan(**{**BASE, field: value})

    def test_root_size_override_is_validated(self):
        plan = planning.auto_plan(**{**BASE, "swap_mib": 1024, "root_mib": 8192}) \
            if False else planning.InstallPlan(
                disk="/dev/sda",
                firmware="uefi",
                hostname="unifyos",
                timezone="UTC",
                locale="en_US.UTF-8",
                keymap="us",
                username="unify",
                full_name="UnifyOS User",
                swap_mib=1024,
                root_mib=8192,
                disk_mib=64 * 1024,
            )
        self.assertEqual(plan.root_mib, 8192)
        with self.assertRaises(planning.PlanError):
            planning.InstallPlan(
                disk="/dev/sda",
                firmware="uefi",
                hostname="unifyos",
                timezone="UTC",
                locale="en_US.UTF-8",
                keymap="us",
                username="unify",
                full_name="UnifyOS User",
                swap_mib=1024,
                root_mib=1024,
                disk_mib=64 * 1024,
            )


class LayoutTests(unittest.TestCase):
    def test_partition_device_naming(self):
        self.assertEqual(planning.partition_device("/dev/sda", 1), "/dev/sda1")
        self.assertEqual(planning.partition_device("/dev/nvme0n1", 2), "/dev/nvme0n1p2")
        self.assertEqual(planning.partition_device("/dev/mmcblk0", 1), "/dev/mmcblk0p1")

    def test_uefi_layout_indexes(self):
        layout = planning.partition_layout(planning.auto_plan(**BASE))
        self.assertEqual(layout.esp, "/dev/sda1")
        self.assertEqual(layout.swap, "/dev/sda2")
        self.assertEqual(layout.root, "/dev/sda3")

    def test_partition_commands_are_gpt_typed(self):
        commands = planning.partition_commands(planning.auto_plan(**BASE))
        self.assertEqual(commands[0], ["sgdisk", "--zap-all", "/dev/sda"])
        joined = [" ".join(command) for command in commands]
        self.assertTrue(any("ef00" in command for command in joined))
        self.assertTrue(any("8200" in command for command in joined))
        self.assertTrue(any("8300" in command for command in joined))

    def test_format_commands(self):
        commands = planning.format_commands(planning.auto_plan(**BASE))
        joined = [" ".join(command) for command in commands]
        self.assertTrue(any(command.startswith("mkfs.fat") for command in joined))
        self.assertTrue(any(command.startswith("mkswap") for command in joined))
        self.assertTrue(any(command.startswith("mkfs.ext4") for command in joined))

    def test_fstab_uses_uuids(self):
        content = planning.fstab(
            planning.auto_plan(**BASE),
            {"root": "root-uuid", "esp": "esp-uuid", "swap": "swap-uuid"},
        )
        self.assertIn("UUID=root-uuid\t/\text4", content)
        self.assertIn("/boot/efi", content)
        self.assertIn("swap", content)


class DiskSafetyTests(unittest.TestCase):
    def test_system_disks_are_detected(self):
        disks = system_disks(MOUNTINFO)
        self.assertIn("nvme0n1", disks)
        self.assertIn("sda", disks)
        self.assertNotIn("tmpfs", disks)

    def test_running_system_disk_is_refused(self):
        with self.assertRaises(SafetyError):
            check_install_target(
                "/dev/nvme0n1",
                size_bytes=64 * 1024**3,
                removable=False,
                mountinfo=MOUNTINFO,
            )

    def test_small_target_is_refused(self):
        with self.assertRaises(SafetyError):
            check_install_target(
                "/dev/sdb", size_bytes=2 * 1024**3, removable=False, mountinfo=""
            )

    def test_removable_target_needs_opt_in(self):
        with self.assertRaises(SafetyError):
            check_install_target(
                "/dev/sdb", size_bytes=64 * 1024**3, removable=True, mountinfo=""
            )
        check_install_target(
            "/dev/sdb",
            size_bytes=64 * 1024**3,
            removable=True,
            mountinfo="",
            allow_removable=True,
        )

    def test_clean_disk_is_accepted(self):
        check_install_target(
            "/dev/sdb", size_bytes=64 * 1024**3, removable=False, mountinfo=MOUNTINFO
        )


class DryRunTests(unittest.TestCase):
    def test_dry_run_records_commands_without_touching_the_disk(self):
        plan = planning.auto_plan(**{**BASE, "disk": "/dev/sdz"})
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "target"
            installer = Installer(
                plan,
                target=str(target),
                dry_run=True,
                log_path=str(Path(directory) / "install.log"),
            )
            commands = installer.run(password="secret-password")
            joined = [" ".join(command) for command in commands]
            self.assertEqual(joined[0], "sgdisk --zap-all /dev/sdz")
            self.assertTrue(any(command.startswith("mkfs.ext4") for command in joined))
            self.assertTrue(any("grub-install" in command for command in joined))
            self.assertTrue(any("chpasswd" in command for command in joined))
            self.assertFalse(target.exists(), "dry run must not create the target")
            self.assertTrue((Path(directory) / "install.log").exists())


if __name__ == "__main__":
    unittest.main()
