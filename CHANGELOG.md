# Changelog

All notable changes to UnifyOS. The `VERSION` file is the single source of
truth for the distribution version.

## [0.1.1] — 2026-11-09

Fixes the boot failure reported from a Hyper-V virtual machine and documents the
session accounts.

### Fixed

* **Kernel panic `Attempted to kill init` on Hyper-V generation-2 machines.**
  Two defects combined:
  * The initramfs carried no drivers for the Hyper-V synthetic SCSI controller,
    so the DVD never appeared as `/dev/sr0` and the live medium was never found.
    It now ships `hv_vmbus`, `hv_storvsc`, `hyperv_keyboard` and `hid_hyperv`,
    and waits up to 15 seconds for block devices to appear, because storage
    drivers probe asynchronously on several buses.
  * The no-medium path ended in `exec /bin/sh`. When that shell received end of
    input it exited 0, and PID 1 exiting panics the kernel at 0.48 s with the
    exact message from the report. `iso/live/init` now restarts the rescue shell
    in a loop, creates `/dev/console` itself when devtmpfs did not, prints the
    visible block devices and the tail of `dmesg`, and cannot return at all.
* **Live-root drivers.** `rcS` loads input, network and virtualisation drivers
  by path from `/etc/unify/live-modules.load` (Hyper-V keyboard and network,
  virtio, USB HID/storage, common wired NICs), since a read-only squashfs has no
  module index and `depmod` cannot run inside it.
* **Stale GRUB title.** The minimal menu entry hard-coded `0.1.0`; the version is
  now only in `/etc/os-release` and `.disk/info`.

### Added

* `scripts/test-live-init.sh` — runs the real `iso/live/init` in a mount
  namespace with no live medium and fails unless it is still running when the
  timeout fires. Wired into CI before the image build.
* `scripts/qemu-boot-test.sh` — the boot test now also fails on `Kernel panic`
  / `Attempted to kill init` even when the session markers appear, and boots the
  image a third time from a virtio-scsi CD to exercise a non-AHCI transport.

### Changed

* **Documented credentials.** `unify` / `unify` with `sudo`; direct root login is
  locked and the live console is already a root shell (see `README.md`).

### Artifacts

* `UnifyOS-0.1.1-minimal-amd64.iso` — 293 MiB (307,232,768 bytes) hybrid
  BIOS+UEFI live image, published at
  https://github.com/itriedcoding/UnifyOS/releases/tag/v0.1.1 with SHA256
  `f9c4055fce8ef395e728fc45e6c88e2d7431299aa424d59f51ab837e7c79ed04`.
  CI booted it under SeaBIOS, under OVMF and from a virtio-scsi CD before the
  release step ran; the initramfs carries 46 modules and the live root loads 41
  more by path.

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

* `UnifyOS-0.1.0-minimal-amd64.iso` — 292 MiB hybrid BIOS+UEFI live image,
  published at https://github.com/itriedcoding/UnifyOS/releases/tag/v0.1.0
  with SHA256
  `be72b9970380826fed72befb4dcbccece0004708a1076e0981ccdf1a41d67797`.
  Boot-tested by the CI that built it: both SeaBIOS and OVMF reached the live
  session before the release step ran. It did not boot on Hyper-V; see 0.1.1.

### Not included

* Desktop root filesystem and desktop ISO (needs a privileged host).
* Graphical login screen, signed package repository, performance measurements.
