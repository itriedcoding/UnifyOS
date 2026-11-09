# UnifyOS

A lightweight Debian-based Linux distribution: a curated Wayland desktop, a
GTK4 application suite, small Rust system services, a scripted hybrid ISO
pipeline and a Python installer backend with a graphical front end.

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
| `iso/build-live.sh` + `iso/make-iso.sh` - live image and hybrid ISO | Implemented | built and booted under BIOS and UEFI in CI |
| Desktop configuration (labwc, waybar, fuzzel, mako), themes, branding | Implemented | configuration files |

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

Latest published build, produced and tested by CI:

```
UnifyOS-0.1.0-minimal-amd64.iso        292 MiB
sha256 be72b9970380826fed72befb4dcbccece0004708a1076e0981ccdf1a41d67797
El Torito: BIOS (eltorito.img) + UEFI (efi.img)
```

Both firmware types reached the live session during that build, under SeaBIOS
and OVMF. Download it from the
[v0.1.0 release](https://github.com/itriedcoding/UnifyOS/releases/tag/v0.1.0).

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

`.github/workflows/iso.yml` builds the image on a Linux runner, boots it under
SeaBIOS and OVMF in QEMU, and publishes it. A failed boot stops the job before
the release step, so a published image is one that reached the live session.

```bash
git tag v0.1.0 && git push origin v0.1.0   # build + boot test + release
```

A manual run keeps the image as a workflow artifact instead of a release.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
