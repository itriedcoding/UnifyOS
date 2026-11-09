# UnifyOS

A lightweight Debian-based Linux distribution under construction: a curated
Wayland desktop, small Rust system services, a scripted hybrid ISO pipeline and
a Python installer engine.

**Read [documentation/limitations.md](documentation/limitations.md) before
evaluating this project.** The foundation is real and tested; the custom GUI
application suite and the graphical installer are not implemented yet, and no
ISO was produced inside the unprivileged development container used to write
this code.

## What exists and is verified

| Component | State | Verification |
|---|---|---|
| `system/unify-core` — procfs/sysfs parsers, JSON writer, exec/FS helpers | Implemented | `cargo test` (unit tests on fixture data) |
| `system/unify-ctl` — system control CLI (info, power, brightness, audio, network, Bluetooth, displays, processes) | Implemented | `cargo build`; commands exercise real interfaces |
| `system/unify-sysmond` — sampling daemon writing `/run/unify/*.json` | Implemented | builds; bounded by a systemd unit |
| `system/unify-pkg` — apt front-end with essential-package protection and audit log | Implemented | builds; policy logic unit-reviewable |
| `installer/` — validated install plans, GPT layout, safety checks, execution engine, CLI | Implemented | `python3 -m unittest` incl. dry-run command sequence |
| `iso/make-iso.sh` — hybrid BIOS+UEFI ISO assembly (squashfs + GRUB + xorriso) | Implemented | shell-checked; not executed end-to-end here |
| `iso/bootstrap-rootfs.sh` — rootfs bootstrap, privileged and chrootless | Implemented | chrootless mode built a 177 MB Debian base rootfs in this container |
| Desktop configuration (labwc, waybar, fuzzel, mako), themes, udev rules, systemd units | Implemented | configuration files, not boot-tested |

## Not implemented

* The nine custom GUI applications (Settings, Store, Control Center, System
  Monitor, File Manager, Terminal, Updater, Welcome, Recovery) and the
  graphical installer.
* A graphical login screen (the session is autologin + `swaylock`).
* Signed package repository generation.
* Boot, installer and performance testing on real or virtual hardware.

See [documentation/limitations.md](documentation/limitations.md) for the
detailed list and the reasons.

## Layout

```
system/       Rust workspace: unify-core, unify-ctl, unify-sysmond, unify-pkg
installer/    Python installer engine, CLI and unit tests
iso/          rootfs bootstrap, package profiles, GRUB menus, ISO assembler
desktop/      labwc/waybar/fuzzel/mako session configuration (installed to /etc/skel)
system/units  systemd units shipped in the image
system/udev   udev rules (backlight access for the video group)
themes/       GTK and labwc colour schemes
wallpapers/   desktop backgrounds
branding/     logo and identity assets
documentation/ limitations.md — what is verified, what is not, and why
```

## Building and testing

```bash
make system          # build the Rust components (release)
make test            # Rust unit tests + installer tests + shell syntax
./iso/bootstrap-rootfs.sh --edition desktop   # needs root or user namespaces
./iso/make-iso.sh --rootfs build/rootfs-desktop --edition desktop
```

The ISO stage needs a host with `squashfs-tools`, `xorriso`, `grub-pc-bin`,
`grub-efi-amd64-bin` and `mtools`. On a privileged machine that is simply
`make iso-desktop`.

## Using the Rust tools

```bash
unify-ctl sysinfo --json
unify-ctl cpu
unify-ctl brightness set 40
unify-ctl volume set 65
unify-ctl network wifi-list --json
unify-ctl bluetooth status
unify-ctl processes --sort mem --limit 10 --json
unify-pkg upgradable --json
unify-installer --list-disks
unify-installer --disk /dev/sda --dry-run
```

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
