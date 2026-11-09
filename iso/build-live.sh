#!/usr/bin/env bash
# UnifyOS live image builder.
#
# Assembles a bootable live system from a root filesystem plus a kernel tree and
# busybox, then hands over to iso/make-iso.sh for the hybrid BIOS+UEFI image.
#
#   ./iso/build-live.sh                 # minimal console image
#   EDITION=desktop ./iso/build-live.sh # when a desktop rootfs is available
#
# On a normal Linux host the tools come from the system (squashfs-tools,
# xorriso, grub-pc-bin, grub-efi-amd64-bin, mtools, busybox-static). In the
# unprivileged container the same tools are extracted under build/tools and
# build/mtools; they are used automatically when present.
#
# Environment: KERNEL_DIR BASE_ROOTFS BUSYBOX TOOLS MTOOLS OUT SQUASH_LEVEL EDITION

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EDITION="${EDITION:-minimal}"
VERSION="$(cat "$ROOT/VERSION")"

KERNEL_DIR="${KERNEL_DIR:-$ROOT/build/kernel}"
BASE_ROOTFS="${BASE_ROOTFS:-$ROOT/build/rootfs4}"
TOOLS="${TOOLS:-$ROOT/build/tools/hostroot}"
MTOOLS="${MTOOLS:-$ROOT/build/mtools/usr/bin}"
SQUASH_LEVEL="${SQUASH_LEVEL:-3}"
OUT="${OUT:-$ROOT/out/UnifyOS-$VERSION-$EDITION-amd64.iso}"
WORK="$ROOT/build/live-$EDITION"

# --- tool resolution -------------------------------------------------------
if [ -d "$TOOLS/usr/bin" ]; then
    MKSQUASHFS="$TOOLS/usr/bin/mksquashfs"
    XORRISO="$TOOLS/usr/bin/xorriso"
    GRUB_MKIMAGE="$TOOLS/usr/bin/grub-mkimage"
    GRUB_MKSTANDALONE="$TOOLS/usr/bin/grub-mkstandalone"
    GRUB_LIB="$TOOLS/usr/lib/grub"
    export LD_LIBRARY_PATH="$TOOLS/usr/lib/x86_64-linux-gnu:$TOOLS/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export PATH="$TOOLS/usr/bin:$PATH"
else
    MKSQUASHFS="$(command -v mksquashfs || true)"
    XORRISO="$(command -v xorriso || true)"
    GRUB_MKIMAGE="$(command -v grub-mkimage || true)"
    GRUB_MKSTANDALONE="$(command -v grub-mkstandalone || true)"
    GRUB_LIB="/usr/lib/grub"
fi

if [ -x "$MTOOLS/mformat" ]; then
    MFORMAT="$MTOOLS/mformat"
    MMD="$MTOOLS/mmd"
    MCOPY="$MTOOLS/mcopy"
    export PATH="$MTOOLS:$PATH"
else
    MFORMAT="$(command -v mformat || true)"
    MMD="$(command -v mmd || true)"
    MCOPY="$(command -v mcopy || true)"
fi

if [ -z "${BUSYBOX:-}" ]; then
    if [ -x "$ROOT/build/busybox-root/bin/busybox" ]; then
        BUSYBOX="$ROOT/build/busybox-root/bin/busybox"
    else
        BUSYBOX="$(command -v busybox || true)"
    fi
fi

for path in "$KERNEL_DIR" "$BASE_ROOTFS"; do
    if [ ! -d "$path" ]; then
        echo "error: missing required directory: $path" >&2
        exit 1
    fi
done
for tool in "$MKSQUASHFS" "$XORRISO" "$GRUB_MKIMAGE" "$GRUB_MKSTANDALONE" \
            "$MFORMAT" "$MMD" "$MCOPY" "$BUSYBOX"; do
    if [ -z "$tool" ] || [ ! -x "$tool" ]; then
        echo "error: required tool not found: '${tool:-?}'" >&2
        echo "       install squashfs-tools xorriso grub-pc-bin grub-efi-amd64-bin mtools busybox-static" >&2
        exit 1
    fi
done

KERNEL="$(ls "$KERNEL_DIR"/boot/vmlinuz-* 2>/dev/null | head -1)"
[ -n "$KERNEL" ] || { echo "error: no vmlinuz in $KERNEL_DIR/boot" >&2; exit 1; }
KVER="${KERNEL##*/vmlinuz-}"
MODDIR="$KERNEL_DIR/lib/modules/$KVER"

echo "[1/6] building release system binaries"
if [ ! -x "$ROOT/system/target/release/unify-ctl" ]; then
    ( cd "$ROOT/system" && cargo build --release --offline )
fi

echo "[2/6] staging live root filesystem from $BASE_ROOTFS"
rm -rf "$WORK"
mkdir -p "$WORK/root" "$WORK/initramfs"
# Device nodes and virtual filesystems in the base image cannot be recreated
# without privileges, and the live system mounts its own, so they are left out
# and recreated empty just below.
if command -v rsync >/dev/null 2>&1; then
    rsync -a --exclude=/dev/* --exclude=/proc/* --exclude=/sys/* --exclude=/run/* \
        "$BASE_ROOTFS"/ "$WORK/root"/
else
    ( cd "$BASE_ROOTFS" && tar -cf - \
        --exclude=./dev --exclude=./proc --exclude=./sys --exclude=./run . ) \
        | ( cd "$WORK/root" && tar -xpf - )
fi
mkdir -p "$WORK/root"/{proc,sys,dev,run,tmp,var/tmp,var/log,mnt,media,boot/grub,etc/unify}
install -m644 "$KERNEL" "$WORK/root/boot/vmlinuz-$KVER"

# Modules are hard-linked when possible: same filesystem, no extra disk use.
if [ -d "$MODDIR" ]; then
    mkdir -p "$WORK/root/lib/modules"
    cp -al "$MODDIR" "$WORK/root/lib/modules/$KVER" 2>/dev/null \
        || cp -a "$MODDIR" "$WORK/root/lib/modules/$KVER"
fi

for binary in unify-ctl unify-sysmond unify-pkg; do
    source_binary="$ROOT/system/target/release/$binary"
    if [ -x "$source_binary" ]; then
        install -m755 "$source_binary" "$WORK/root/usr/bin/$binary"
    fi
done

echo "[3/6] installing busybox init and UnifyOS userspace"
install -m755 "$BUSYBOX" "$WORK/root/bin/busybox"
ln -sfn ../bin/busybox "$WORK/root/sbin/init"
for applet in ip mount umount mdev hostname cat clear sleep stty switch_root insmod lsmod dmesg; do
    ln -sfn busybox "$WORK/root/bin/$applet"
done

install -D -m755 "$ROOT/iso/live/rcS" "$WORK/root/etc/unify/rcS"
install -m644 "$ROOT/iso/live/inittab" "$WORK/root/etc/inittab"

cat > "$WORK/root/etc/os-release" <<EOF
NAME="UnifyOS"
ID=unifyos
ID_LIKE=debian
VERSION="$VERSION"
VERSION_ID="$VERSION"
PRETTY_NAME="UnifyOS $VERSION ($EDITION)"
HOME_URL="https://github.com/itriedcoding/UnifyOS"
ANSI_COLOR="1;34"
EOF

cat > "$WORK/root/etc/unify-release" <<EOF

   UnifyOS $VERSION — $EDITION live system
   Debian 12 base, kernel $KVER

   This image boots to a shell. Run 'unify-ctl sysinfo' for hardware
   information and see /usr/share/doc/unify for documentation.

EOF

if [ -d "$ROOT/system/units" ]; then
    install -d "$WORK/root/etc/systemd/system"
    for unit in "$ROOT/system/units"/*.service; do
        [ -f "$unit" ] && install -m644 "$unit" "$WORK/root/etc/systemd/system/"
    done
fi

echo "[4/6] building initramfs (kernel $KVER)"
IR="$WORK/initramfs"
mkdir -p "$IR"/{bin,dev,proc,sys,media,root,lib/modules}
install -m755 "$BUSYBOX" "$IR/bin/busybox"
for applet in sh mount umount insmod switch_root sleep echo cat mkdir readlink; do
    ln -sfn busybox "$IR/bin/$applet"
done
install -m755 "$ROOT/iso/live/init" "$IR/init"

if [ -d "$MODDIR" ]; then
    python3 "$ROOT/scripts/module-closure.py" --modules-dir "$MODDIR" \
        squashfs isofs loop > "$WORK/modules.list"
    : > "$IR/modules.load"
    while IFS= read -r relative; do
        [ -n "$relative" ] || continue
        mkdir -p "$IR/lib/modules/$KVER/$(dirname "$relative")"
        cp "$MODDIR/$relative" "$IR/lib/modules/$KVER/$relative"
        printf 'lib/modules/%s/%s\n' "$KVER" "$relative" >> "$IR/modules.load"
    done < "$WORK/modules.list"
    echo "      $(wc -l < "$IR/modules.load") modules in initramfs"
else
    echo "warning: no module tree at $MODDIR; the initramfs will have no storage modules" >&2
    : > "$IR/modules.load"
fi

( cd "$IR" && find . | "$BUSYBOX" cpio -o -H newc 2>/dev/null ) \
    | gzip -9 > "$WORK/root/boot/initrd.img-$KVER"
echo "      initramfs: $(du -h "$WORK/root/boot/initrd.img-$KVER" | cut -f1)"

echo "[5/6] assembling ISO"
SQUASH_LEVEL="$SQUASH_LEVEL" \
MKSQUASHFS="$MKSQUASHFS" \
XORRISO="$XORRISO" \
GRUB_MKIMAGE="$GRUB_MKIMAGE" \
GRUB_MKSTANDALONE="$GRUB_MKSTANDALONE" \
GRUB_LIB="$GRUB_LIB" \
MFORMAT="$MFORMAT" MMD="$MMD" MCOPY="$MCOPY" \
    "$ROOT/iso/make-iso.sh" --rootfs "$WORK/root" --edition "$EDITION" \
        --output "$OUT" --compression zstd

echo "[6/6] verifying image structure"
"$XORRISO" -indev "$OUT" -report_el_torito plain 2>/dev/null | sed -n '1,20p' || true

echo
echo "Built $OUT"
ls -lh "$OUT"
