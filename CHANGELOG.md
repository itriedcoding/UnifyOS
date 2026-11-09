# Changelog

All notable changes to UnifyOS. The `VERSION` file is the single source of
truth for the distribution version; `system/Cargo.toml` and the installer
package follow it.

## [0.1.0] — 2026-11-09

First foundation release. This is **not** a usable daily-driver system yet;
read `documentation/limitations.md` before evaluating.

### Added

* **Rust system layer** (`system/`, no external crates):
  * `unify-core`: `/proc` and sysfs parsers (CPU, memory, load, diskstats,
    processes, hwmon sensors, batteries, backlights), mount-table and
    block-device discovery, `statvfs` bindings, process execution helpers and a
    JSON writer.
  * `unify-ctl`: system control CLI — `sysinfo`, `cpu`, `mem`, `disk`,
    `sensors`, `battery`, `power`, `brightness`, `volume`, `network`,
    `bluetooth`, `display`, `processes`, with `--json` for the desktop.
  * `unify-sysmond`: bounded sampling daemon writing atomic snapshots and a
    60-sample history ring to `/run/unify/`.
  * `unify-pkg`: apt front-end with essential-package protection, an audit log
    at `/var/log/unify/pkg.log`, `--json` output and a package-database check.
* **Installer engine** (`installer/`): validated install plans (GPT layout,
  UEFI and BIOS), disk-safety refusals (system disk, removable media, minimum
  size), sgdisk/mkfs/fstab generation, rsync + chroot + GRUB execution,
  logging, dry-run mode, CLI front end and unit tests.
* **ISO pipeline** (`iso/`): desktop and minimal package profiles, rootfs
  bootstrap for privileged and chrootless builds, image hooks (users, services,
  firewall, sysctl hardening, initramfs, splash), hybrid BIOS+UEFI ISO
  assembly with squashfs and GRUB, SHA256 checksums.
* **Desktop configuration** (`desktop/`): labwc window manager with Unify
  keybindings, waybar panel, fuzzel launcher, mako notifications, session
  autostart, installed through `/etc/skel`.
* **Branding and themes**: `unify-dark` and `unify-light` colour schemes,
  logo, wallpapers, Plymouth splash theme.
* **System integration**: systemd units for the sampler and the desktop
  session, udev rule for backlight access, default `unify.toml` configuration.
* **Documentation**: this changelog, `documentation/limitations.md` and the
  project README.

### Not included (see limitations)

* Custom GTK4 applications (Settings, Store, Control Center, Monitor, File
  Manager, Terminal, Updater, Welcome, Recovery).
* Graphical installer and graphical login greeter.
* Signed package repository.
* Boot, install and performance test results on real or virtual hardware.
