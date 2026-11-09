# UnifyOS — limitations and status

This document records what is verified, what is not, and why. It is updated
with every change; treat anything not listed as verified as untested.

## Verified in this environment

The build container is an unprivileged Linux sandbox (no `root`, no
`CAP_SYS_ADMIN`, no `/dev/kvm`, no QEMU, 2 vCPUs, ~4 GB RAM).

Verified by running the code:

* **Rust system layer** — `cargo test`: 27 unit tests covering `/proc` and sysfs
  parsing, the JSON writer, mount parsing and filesystem helpers. All four
  binaries build in release mode.
* **Installer backend** — `python3 -m unittest`: 19 tests covering plan
  validation, GPT layout, partition naming, command generation, fstab output,
  disk-safety refusals and the complete dry-run command sequence.
* **Application suite** — all ten GTK4 programs compile against GTK4 4.8 with
  `-Wall -Wextra` and produce zero warnings.
* **Live ISO** - CI builds a 292 MiB hybrid image on `ubuntu-24.04`; `xorriso`
  reports both El Torito entries (BIOS `eltorito.img`, UEFI `efi.img`), the
  squashfs is 266 MB with `-all-root` ownership normalisation, and a SHA256
  checksum is written next to the image.
* **The minimal ISO boots.** `scripts/qemu-boot-test.sh` booted
  `UnifyOS-0.1.0-minimal-amd64.iso` under SeaBIOS and under OVMF; both cases
  reached the live session, printing `unify-live: live medium found` and
  `Live session ready` on the serial console. That build is published at
  https://github.com/itriedcoding/UnifyOS/releases/tag/v0.1.0.

## Not verified

* **Only the minimal console image is boot-tested.** The boot test proves GRUB,
  initramfs module loading, squashfs `switch_root` and busybox init reach a
  session on both firmwares. It does not exercise a desktop session, hardware
  beyond the emulated QEMU devices, or booting from a USB stick.
* **No application has been launched.** GTK4 applications need a display
  server; this container has none. Compilation proves the code is well formed
  and links against GTK4, not that the windows behave correctly.
* **No desktop root filesystem was produced.** `iso/profiles/desktop.list`
  selects the packages, but the full profile fails during unprivileged
  `chrootless` configuration: several Debian maintainer scripts write to
  absolute paths and `chown` to unmapped group IDs. The ISO builder therefore
  supports `EDITION=desktop` but the desktop image cannot be produced without a
  privileged host. The minimal image contains no GTK4 runtime libraries and no
  desktop session, so it boots to a console only.
* **The installer has never written to a disk.** It requires root and a spare
  block device. Its plan generation, safety refusals and command sequence are
  unit-tested and dry-run verified; partitioning, formatting and GRUB
  installation are not.
* **No graphical login screen.** The desktop session is autologin on tty1 with
  `swaylock` for locking.
* **No signed package repository.** `packages/` was scoped but not built.
* **Performance targets are unmeasured.** 150–300 MB idle RAM, 2–4 GB installed
  size and <10 s boot are goals, not results. No numbers are claimed.
* **Security hardening is configured, not audited.** The image hooks enable
  AppArmor, UFW and sysctl hardening; no audit or penetration test was done.

## Known technical limitations

* The minimal image's kernel modules are hard-linked from the Debian kernel
  package; because `depmod` cannot run without root, the live root filesystem
  has no `modules.dep`, so `modprobe` by name will not work inside it. The
  initramfs carries an explicit, dependency-ordered module list instead.
* `applications/build.sh` links with `-Wl,--allow-shlib-undefined` because the
  extracted GTK4 sysroot has no linker scripts for cairo's private
  dependencies. Symbols resolve at run time against the same libraries; this is
  a build-environment workaround, not a code change.
* `scripts/grub-lists.py` and `scripts/module-closure.py` exist only because
  Debian ships GRUB index files and `modules.dep` as generated build output.
  On a normal system `grub-mkrescue` and `depmod` replace them.

## Boot testing

`scripts/qemu-boot-test.sh` boots an image under SeaBIOS and OVMF and exits
non-zero unless the serial console reaches the live session. CI runs it on every
tag and every manual build, and the v0.1.0 image passed both cases. Both serial
logs are kept as the `boottest-logs` artifact, so a failure names the missing
marker and prints the tail of the console.

## Known portability requirements

* The installer requires Python 3.9 or newer: it uses `zoneinfo` for timezone
  validation. Debian 12 ships 3.11, so the image is unaffected; a Windows
  Python 3.8 cannot import it.
* The Rust system layer reads `/proc` and `/sys` and is Linux-only by design.
* The GTK4 applications need `libgtk-4-1` and a display server at run time.

## Next steps, in order

1. Build the desktop root filesystem on a privileged host, integrate the GTK4
   applications and their runtime libraries, and boot the desktop image.
2. Run the graphical installer against a spare disk and boot the result.
3. Measure idle memory, installed size and boot time on documented hardware.
4. Boot the published image on real hardware and from a USB stick.
