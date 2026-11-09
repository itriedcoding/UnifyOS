# UnifyOS — limitations and status

This document records what is verified, what is not, and why. It is updated
with every change; treat anything not listed as verified as untested.

## Verified in this environment

The build container is an unprivileged Linux sandbox (no `root`, no
`CAP_SYS_ADMIN`, no `/dev/kvm`, no QEMU, 2 vCPUs, ~4 GB RAM). Everything that
needs root or an emulator runs on the CI runner instead.

Verified by running the code:

* **Rust system layer** — `cargo test`: 27 unit tests covering `/proc` and sysfs
  parsing, the JSON writer, mount parsing and filesystem helpers. All four
  binaries build in release mode.
* **Installer backend** — `python3 -m unittest`: 19 tests covering plan
  validation, GPT layout, partition naming, command generation, fstab output,
  disk-safety refusals and the complete dry-run command sequence.
* **Application suite** — all ten GTK4 programs compile against GTK4 4.8 with
  `-Wall -Wextra` and produce zero warnings.
* **Live init cannot exit.** `scripts/test-live-init.sh` runs the real
  `iso/live/init` in a private mount namespace with no live medium and requires
  it to still be running after 8 seconds, so PID 1 survives the failure path
  that panicked a virtual machine.
* **Live ISO** — CI builds a hybrid image on `ubuntu-24.04`; `xorriso` reports
  both El Torito entries (BIOS `eltorito.img`, UEFI `efi.img`), the squashfs is
  written with `-all-root` ownership normalisation, and a SHA256 checksum is
  written next to the image.
* **The minimal ISO boots.** `scripts/qemu-boot-test.sh` boots
  `UnifyOS-0.1.1-minimal-amd64.iso` three times — SeaBIOS, OVMF and a
  virtio-scsi CD — and each case must print `unify-live: live medium found` and
  `Live session ready` on the serial console with no kernel panic in the log.
  The v0.1.0 image passed the first two; v0.1.1 adds the third.

## Not verified

* **The Hyper-V fix has not been booted on Hyper-V.** QEMU cannot emulate the
  synthetic SCSI controller, so v0.1.1 ships `hv_vmbus`/`hv_storvsc`, waits for
  the device node and proves the init survives a missing medium — but the
  reported machine itself has not been retested. Confirm with the release image
  and report the rescue shell's block-device list if it still fails.
* **Only the minimal console image is boot-tested.** The boot test proves GRUB,
  initramfs module loading, squashfs `switch_root` and busybox init reach a
  session on three emulated transports. It does not exercise a desktop session,
  hardware beyond the emulated devices, or booting from a USB stick.
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
* **Credentials are public by design.** `unify` / `unify` is documented in
  `README.md` and readable from the ISO by anyone. Direct root login is locked;
  change the password with `passwd` on any installed system.
* **No graphical login screen.** The desktop session is autologin on tty1 with
  `swaylock` for locking.
* **No signed package repository.** `packages/` was scoped but not built.
* **Performance targets are unmeasured.** 150–300 MB idle RAM, 2–4 GB installed
  size and <10 s boot are goals, not results. No numbers are claimed.
* **Security hardening is configured, not audited.** The image hooks enable
  AppArmor, UFW and sysctl hardening; no audit or penetration test was done.

## Known technical limitations

* The live root has no module index: `depmod` cannot write to a read-only
  squashfs. The initramfs therefore carries a dependency-ordered module list,
  and the running system loads its remaining drivers by absolute path from
  `/etc/unify/live-modules.load` instead of by module name.
* The live image uses busybox init, not systemd. The Debian packages installed
  into the root filesystem are configured for systemd, and their units are
  present, but a live session does not start them; only `rcS` runs.
* `applications/build.sh` links with `-Wl,--allow-shlib-undefined` because the
  extracted GTK4 sysroot has no linker scripts for cairo's private
  dependencies. Symbols resolve at run time against the same libraries; this is
  a build-environment workaround, not a code change.
* `scripts/grub-lists.py` and `scripts/module-closure.py` exist only because
  Debian ships GRUB index files and `modules.dep` as generated build output.
  On a normal system `grub-mkrescue` and `depmod` replace them.

## Boot testing

`scripts/qemu-boot-test.sh` boots an image under SeaBIOS, under OVMF and from a
virtio-scsi CD, and exits non-zero unless the serial console reaches the live
session and stays free of `Kernel panic` / `Attempted to kill init`. CI runs it
on every tag and every manual build. All serial logs are kept as the
`boottest-logs` artifact, so a failure names the missing marker and prints the
tail of the console. `scripts/test-live-init.sh` runs earlier in the same job and
covers the case where no medium is found at all.

## Known portability requirements

* The installer requires Python 3.9 or newer: it uses `zoneinfo` for timezone
  validation. Debian 12 ships 3.11, so the image is unaffected; a Windows
  Python 3.8 cannot import it.
* The Rust system layer reads `/proc` and `/sys` and is Linux-only by design.
* The GTK4 applications need `libgtk-4-1` and a display server at run time.

## Next steps, in order

1. Reboot the reported Hyper-V generation-2 machine from the v0.1.1 image and
   confirm the live session.
2. Build the desktop root filesystem on a privileged host, integrate the GTK4
   applications and their runtime libraries, and boot the desktop image.
3. Run the graphical installer against a spare disk and boot the result.
4. Measure idle memory, installed size and boot time on documented hardware.
5. Boot the published image on real hardware and from a USB stick.
