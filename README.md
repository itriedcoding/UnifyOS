# UnifyOS

A lightweight Debian-based Linux distribution: a curated Wayland desktop, a
GTK4 application suite, small Rust system services, a scripted hybrid ISO
pipeline and a Python installer backend with a graphical front end.

UnifyOS is its own operating system. It is built from Debian packages, it does
not embed, dual-boot with or load any other OS, and the minimal image ships only
what a console session needs — no desktop, no browsers, no office suite, no
service layer on top of another distribution's session.

Read [documentation/limitations.md](documentation/limitations.md) first. It
states exactly what has been verified in this repository and what has not.

## What is implemented

| Component | State | Verification |
|---|---|---|
| `applications/` — nine GTK4 apps and the graphical installer | Implemented | all ten compile against GTK4 4.8 with `-Wall -Wextra`, zero warnings |
| `system/unify-core` — procfs/sysfs parsers, JSON writer, exec helpers | Implemented | 27 unit tests pass |
| `system/unify-ctl` — system control CLI (13 commands, `--json`) | Implemented | builds; exercised against this machine |
| `system/unify-sysmond` — sampling daemon writing `/run/unify/*.json` | Implemented | builds; systemd unit shipped |
| `system/unify-pkg` — apt front-end with essential-package protection | Implemented | builds; `help` verified |
| `installer/` — plan model, disk safety, partitioning, chroot, GRUB | Implemented | 19 unit tests incl. dry-run sequence |
| `installer/unify-installer` — CLI front end for the above | Implemented | dry run verified |
| `iso/build-live.sh` + `iso/make-iso.sh` — live image and hybrid ISO | Implemented | built and booted under BIOS, UEFI and virtio-scsi in CI |
| Desktop configuration (labwc, waybar, fuzzel, mako), themes, branding | Implemented | configuration files |

## Logging in

The live console needs no login: busybox init starts a **root shell** on
`tty1`, `tty2`, `tty3` and on the serial port (`ttyS0`).

The image also contains a normal user account, which is what a login prompt or
an install to disk uses:

| Account | Username | Password | Notes |
|---|---|---|---|
| Live session console | *(already root)* | *(none)* | No login is performed; the console is a root shell |
| User account | `unify` | `unify` | Member of `sudo`, `video`, `audio`, `input`, `netdev` when those groups exist; use `sudo -i` for a root shell |
| Root account | `root` | *(locked)* | Direct root login is disabled; use `sudo -i` from `unify` |

Change the password with `passwd` (or `sudo passwd unify`). These are the
documented credentials of a live image, not a secret: anyone with the ISO can
read them here, so set a new one before using an installed system on a shared
machine.

## The applications

All are GTK4 C applications that call the real backends — nothing is mocked.

| Application | What it does |
|---|---|
| `unify-monitor` | Live CPU, memory, load, uptime and top processes from `unify-ctl` |
| `unify-settings` | System identity, appearance toggle, shortcuts to the panels |
| `unify-control-center` | Volume, backlight and radio toggles via `unify-ctl` |
| `unify-store` | Searches the archive and installs packages via `unify-pkg` |
| `unify-files` | Directory browsing, navigation and opening files |
| `unify-terminal` | Launches the terminal emulator, with a command runner fallback |
| `unify-updater` | Lists upgrades and applies them via `unify-pkg upgrade` |
| `unify-welcome` | First-run orientation with working entry points |
| `unify-recovery` | Read-only diagnostics, package audit, restart/shutdown |
| `unify-installer-gui` | Graphical front end over the audited installer backend |

Build them with:

```bash
./applications/build.sh            # sets up pkg-config for this container
./applications/build.sh install DESTDIR=/some/rootfs
```

## The ISO

A minimal live image is produced by:

```bash
./iso/build-live.sh                # minimal console edition
EDITION=desktop ./iso/build-live.sh
```

It stages a root filesystem, installs busybox init plus the Rust system
binaries, builds an initramfs containing the kernel modules needed to mount the
live medium, squashes the root, and writes a hybrid BIOS+UEFI image with GRUB.

The initramfs carries the storage drivers for every medium the image claims to
boot from: SATA/AHCI and IDE (`sr_mod`, `libata`, `ahci`, `ata_piix`), virtio,
NVMe, USB mass storage and the Hyper-V synthetic SCSI controller (`hv_vmbus`,
`hv_storvsc`) used by Hyper-V generation-2 virtual machines. If a medium is not
found, the init stops with a rescue shell that prints the block devices and the
tail of the kernel log instead of exiting, because an exiting PID 1 panics the
kernel.

Latest published build, produced and tested by CI:

```
UnifyOS-0.1.1-minimal-amd64.iso        293 MiB (307,232,768 bytes)
sha256 f9c4055fce8ef395e728fc45e6c88e2d7431299aa424d59f51ab837e7c79ed04
El Torito: BIOS (eltorito.img) + UEFI (efi.img)
```

Both firmware types and two CD transports reached the live session during that
build, under SeaBIOS and OVMF in QEMU. Download it from the
[v0.1.1 release](https://github.com/itriedcoding/UnifyOS/releases/tag/v0.1.1).

## Troubleshooting

* **`Kernel panic - not syncing: Attempted to kill init`** — this was a real
  defect in 0.1.0 on Hyper-V generation-2 hosts: the initramfs had no
  `hv_storvsc`, so no CD-ROM appeared and the rescue path exited. 0.1.1 ships
  those drivers, waits for the device node and keeps init alive. If you still
  see it with 0.1.1, boot the "verbose boot" entry and report the block devices
  the rescue shell lists.
* **`no live medium with live/filesystem.squashfs found`** — the medium was
  found but the image is not on it, or the controller is not covered. The rescue
  shell lists every visible block device; `mount -o ro /dev/sdX /media` and
  `ls /media/live` shows what the kernel sees.
* **Blank screen on unusual hardware** — choose "no KMS (troubleshooting)" in
  the GRUB menu.

## Building and testing

```bash
make system      # Rust components (release)
make test        # Rust unit tests + installer tests + shell syntax
./applications/build.sh
./iso/build-live.sh
```

On a normal system the ISO stage needs `squashfs-tools`, `xorriso`,
`grub-pc-bin`, `grub-efi-amd64-bin` and `mtools`. In an unprivileged container
the helpers in `scripts/` reconstruct the GRUB index files and module closure
that Debian's binary packages leave to `depmod`.

## Continuous integration

`.github/workflows/iso.yml` builds the image on a Linux runner, tests the live
init, boots the image under SeaBIOS and OVMF in QEMU plus a virtio-scsi CD, and
publishes it. A failed test or boot stops the job before the release step, so a
published image is one that reached the live session.

```bash
git tag v0.1.1 && git push origin v0.1.1   # build + tests + release
```

A manual run keeps the image as a workflow artifact instead of a release.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
