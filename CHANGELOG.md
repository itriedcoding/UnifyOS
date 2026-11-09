# Changelog

All notable changes to UnifyOS. The `VERSION` file is the single source of
truth for the distribution version.

## [0.1.0] — 2026-11-09

Foundation release with a GTK4 application suite and a bootable-image pipeline.

### Added

* **GTK4 application suite** (`applications/`), all C against GTK4 4.8, all
  compiled with `-Wall -Wextra` and no warnings:
  * `unify-monitor` — live CPU, memory, load, uptime and process list.
  * `unify-settings` — system identity, dark-theme toggle and panel shortcuts.
  * `unify-control-center` — volume, backlight and radio controls.
  * `unify-store` — archive search and package installation.
  * `unify-files` — directory browsing and file opening.
  * `unify-terminal` — terminal-emulator launcher with a command-runner fallback.
  * `unify-updater` — available upgrades and `unify-pkg upgrade`.
  * `unify-welcome` — first-run orientation and entry points.
  * `unify-recovery` — read-only diagnostics, package audit, restart/shutdown.
  * `unify-installer-gui` — graphical front end over the Python installer,
    running it in a worker thread and streaming its log.
  * `libunify` — shared helpers: process execution, JSON scanning, window,
    list-row, heading and log widgets.
* **Live image pipeline**:
  * `iso/build-live.sh` — stages a root filesystem, installs busybox init and
    the Rust system binaries, builds a dependency-ordered initramfs, and calls
    the ISO assembler.
  * `iso/live/init`, `rcS`, `inittab` — initramfs medium discovery, squashfs
    switch_root and early boot.
  * `iso/grub/grub-minimal.cfg` — console-oriented GRUB menu.
  * `scripts/module-closure.py` — resolves kernel module dependencies from
    module metadata, without `depmod`.
  * `scripts/grub-lists.py` — regenerates GRUB's index files from module
    metadata.
  * `iso/make-iso.sh` — fixed for extracted toolchains: format directory for
    `grub-mkstandalone`, FAT without forced FAT32, configurable squashfs level,
    and removal of the `/proc` `/sys` `/dev` exclusions that made a booted root
    unusable.
* **Rust system layer** (`system/`): `unify-core`, `unify-ctl`, `unify-sysmond`,
  `unify-pkg`, with 27 unit tests.
* **Installer backend** (`installer/`): validated plans, GPT layout, disk-safety
  refusals, partitioning/formatting/chroot/GRUB execution, dry-run mode, CLI and
  19 unit tests.
* **Desktop and branding**: labwc/waybar/fuzzel/mako configuration, themes,
  wallpapers, udev rule, systemd units.
* **CI pipeline** (`.github/workflows/iso.yml`): bootstraps the root filesystem,
  builds the ISO, boots it under BIOS and UEFI in QEMU, uploads it as an
  artifact and attaches it to a GitHub release on a `v*` tag. The release step
  runs only after both boot tests pass.
* **Documentation**: README, this changelog and `documentation/limitations.md`.

### Artifacts

* `out/UnifyOS-0.1.0-minimal-amd64.iso` — 189 MB hybrid BIOS+UEFI live image,
  El Torito entries for both firmware types verified, SHA256 published beside
  it. **Not booted**: no virtual machine or hardware was available.

### Not included

* Desktop root filesystem and desktop ISO (needs a privileged host).
* Boot testing of any image.
* Graphical login screen, signed package repository, performance measurements.
