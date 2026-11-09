# UnifyOS — limitations and status

This document exists so nobody mistakes the current state for a finished
distribution. It lists what is implemented, what is verified, and what is
missing.

## Verified in the development container

The container used to write this code is an unprivileged Ubuntu 24.04 sandbox
(no `root`, no `sudo`, no `CAP_SYS_ADMIN`, 2 vCPUs, ~4 GB RAM, 16 GB disk).

Verified by running the code:

* `cargo test` in `system/` — unit tests for `/proc` and sysfs parsers, the JSON
  writer, filesystem helpers and mount parsing pass.
* `cargo build --release` for `unify-ctl`, `unify-sysmond`, `unify-pkg`.
* `python3 -m unittest discover -s installer/tests -t installer` — plan
  validation, GPT layout, partition naming, sgdisk/mkfs command generation,
  fstab generation, disk-safety refusals and the dry-run installation sequence.
* `bash -n` over every build script and hook.
* `mmdebstrap --mode=chrootless` produced a real 177 MB Debian bookworm base
  root filesystem (this is the mechanism `iso/bootstrap-rootfs.sh` uses for
  unprivileged builds).

## Not verified — and why

* **No ISO was assembled in this container.** `iso/make-iso.sh` is complete and
  syntax-checked, but the desktop root filesystem it needs was not produced
  here: the full desktop profile (~96 packages) fails partway through
  `chrootless` package configuration because a handful of Debian maintainer
  scripts (`python3.11-minimal`, `bluez`, `base-files`, `login`) write to
  absolute paths and call `chown` to unmapped group IDs. The workaround used for
  the base rootfs (a no-op `chown` shim plus `mksquashfs -all-root`) gets the
  base system through, but not those postinst scripts. A privileged host, or a
  user namespace with subordinate ID ranges (`newuidmap`), does not have this
  problem and is what the scripts target.
* **No boot test was performed.** The sandbox has no KVM, no `/dev/kvm`, no
  QEMU system emulator and no hardware access. GRUB menu entries, the hybrid
  El Torito layout and the squashfs/initramfs wiring are therefore unproven.
* **The graphical applications do not exist.** Unify Settings, Unify Store,
  Unify Control Center, Unify System Monitor, Unify File Manager, Unify
  Terminal, Unify Updater, Unify Welcome and Unify Recovery are not implemented.
  The backends they would use (`unify-ctl`, `unify-pkg`, `unify-sysmond`, the
  installer engine) are implemented and tested, but shipping empty GTK windows
  was explicitly rejected as fake functionality. The desktop image profile
  therefore relies on real, lightweight substitutes (`foot`, `pcmanfm`,
  `pavucontrol`, `firefox-esr`, `mousepad`, `evince`, `mpv`, `imv`) so that a
  built image would still be usable.
* **There is no graphical installer.** `installer/` provides a validated plan
  model, real partitioning/formatting/mounting/rsync/chroot/GRUB logic and a
  CLI, plus unit tests for the plan and dry-run behaviour. It has never been
  run against a real disk (that needs root and a spare device). Manual
  partitioning of arbitrary existing layouts is not supported; the plan model
  creates the documented GPT layout only.
* **No login screen.** The session is started by a systemd unit on tty1 for the
  `unify` user; `swaylock` provides locking. A graphical greeter is not
  implemented.
* **No signed package repository.** `packages/` was scoped but not built.
* **Performance targets are unmeasured.** The 150–300 MB RAM / 2–4 GB disk /
  <10 s boot goals are engineering targets, not results. Nothing in this
  repository has been benchmarked, and no numbers are claimed.
* **Security hardening is configured, not audited.** The image hook enables
  AppArmor, sets sysctl hardening, enables UFW with deny-incoming defaults and
  keeps service exposure minimal, but no audit or penetration test was done.

## Consequences for evaluating the project

Treat this as a working foundation and build system, not as a usable daily
operating system. The next milestones, in order, are:

1. Build the desktop rootfs on a privileged host (`make rootfs-desktop`), fix
   any package-level issues it exposes, and boot it in QEMU/KVM with both UEFI
   and BIOS firmware.
2. Implement the Unify applications against the existing backends, starting
   with Unify System Monitor (consumes `/run/unify/sysmon.json`) and Unify
   Settings (consumes `unify-ctl`/`unify-pkg`).
3. Add the graphical installer front end over `unify_installer` and run a full
   install on a spare disk, then boot the installed system.
4. Measure idle RAM, disk footprint and boot time on documented hardware and
   publish the results.
