#!/usr/bin/env bash
# UnifyOS ISO assembler.
#
# Turns a prepared root filesystem into a bootable hybrid image that boots on
# UEFI systems and on legacy BIOS machines, from USB or optical media.
#
# This script deliberately does not install packages: bootstrap the rootfs with
# iso/bootstrap-rootfs.sh (or your own Debian tooling) and point --rootfs here.
# Every external tool can be overridden with an environment variable, which is
# how the unprivileged CI build in this repository uses extracted Debian tools.
#
# Host requirements: squashfs-tools, xorriso, grub-pc-bin, grub-efi-amd64-bin,
# mtools.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOTFS=""
OUTPUT=""
LABEL="UNIFYOS"
EDITION="desktop"
COMPRESSION="zstd"
KEEP_WORK=0

usage() {
    cat <<'EOF'
Usage: make-iso.sh --rootfs DIR [options]

  --rootfs DIR       prepared root filesystem (required)
  --edition NAME     desktop | minimal (selects GRUB menu, default: desktop)
  --output FILE      ISO path (default: out/UnifyOS-<edition>.iso)
  --label LABEL      filesystem label (default: UNIFYOS)
  --compression C    squashfs compression: zstd | gzip | xz (default: zstd)
  --keep-work        keep the temporary ISO staging tree

Environment overrides:
  MKSQUASHFS XORRISO GRUB_MKIMAGE GRUB_MKSTANDALONE MFORMAT MMD MCOPY GRUB_LIB
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --rootfs) ROOTFS="$2"; shift 2 ;;
        --edition) EDITION="$2"; shift 2 ;;
        --output) OUTPUT="$2"; shift 2 ;;
        --label) LABEL="$2"; shift 2 ;;
        --compression) COMPRESSION="$2"; shift 2 ;;
        --keep-work) KEEP_WORK=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

if [ -z "$ROOTFS" ]; then
    usage
    exit 2
fi
if [ ! -d "$ROOTFS" ]; then
    echo "error: rootfs '$ROOTFS' does not exist" >&2
    exit 1
fi
OUTPUT="${OUTPUT:-$ROOT/out/UnifyOS-$EDITION.iso}"

MKSQUASHFS="${MKSQUASHFS:-mksquashfs}"
XORRISO="${XORRISO:-xorriso}"
GRUB_MKIMAGE="${GRUB_MKIMAGE:-grub-mkimage}"
GRUB_MKSTANDALONE="${GRUB_MKSTANDALONE:-grub-mkstandalone}"
MFORMAT="${MFORMAT:-mformat}"
MMD="${MMD:-mmd}"
MCOPY="${MCOPY:-mcopy}"
GRUB_LIB="${GRUB_LIB:-/usr/lib/grub}"

for tool in "$MKSQUASHFS" "$XORRISO" "$GRUB_MKIMAGE" "$GRUB_MKSTANDALONE" "$MFORMAT" "$MMD" "$MCOPY"; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "error: required tool not found: $tool" >&2
        exit 1
    fi
done
for module in "$GRUB_LIB/i386-pc/cdboot.img" "$GRUB_LIB/i386-pc/boot_hybrid.img"; do
    if [ ! -f "$module" ]; then
        echo "error: missing GRUB module $module (install grub-pc-bin)" >&2
        exit 1
    fi
done

WORK="$ROOT/build/iso-$EDITION"
rm -rf "$WORK"
mkdir -p "$WORK/isodir/live" "$WORK/isodir/boot/grub" "$WORK/isodir/EFI/BOOT" "$WORK/isodir/.disk"
mkdir -p "$(dirname "$OUTPUT")"

echo "[1/6] locating kernel and initramfs"
KERNEL="$(find "$ROOTFS/boot" -maxdepth 1 -name 'vmlinuz-*' -type f 2>/dev/null | sort -V | tail -1 || true)"
if [ -z "$KERNEL" ]; then
    echo "error: no kernel found in $ROOTFS/boot (run the bootstrap + hooks first)" >&2
    exit 1
fi
KERNEL_VERSION="${KERNEL##*/vmlinuz-}"
INITRAMFS="$ROOTFS/boot/initrd.img-$KERNEL_VERSION"
if [ ! -f "$INITRAMFS" ]; then
    INITRAMFS="$(find "$ROOTFS/boot" -maxdepth 1 -name 'initrd.img-*' -type f 2>/dev/null | sort -V | tail -1 || true)"
fi
if [ ! -f "$INITRAMFS" ]; then
    echo "error: no initramfs found in $ROOTFS/boot" >&2
    exit 1
fi
echo "      kernel $KERNEL_VERSION, initramfs ${INITRAMFS##*/}"

echo "[2/6] building squashfs ($COMPRESSION)"
case "$COMPRESSION" in
    zstd) SQUASH_OPTIONS=(-comp zstd -Xcompression-level "${SQUASH_LEVEL:-15}") ;;
    gzip) SQUASH_OPTIONS=(-comp gzip -Xcompression-level 9) ;;
    xz) SQUASH_OPTIONS=(-comp xz -Xbcj x86) ;;
    *) echo "error: unsupported compression '$COMPRESSION'" >&2; exit 1 ;;
esac
"$MKSQUASHFS" "$ROOTFS" "$WORK/isodir/live/filesystem.squashfs" \
    "${SQUASH_OPTIONS[@]}" -all-root -noappend -no-progress -wildcards \
    -e boot/grub -e var/cache/apt/archives -e var/lib/apt/lists

echo "[3/6] staging boot files"
cp -- "$KERNEL" "$WORK/isodir/live/vmlinuz"
cp -- "$INITRAMFS" "$WORK/isodir/live/initrd.img"
MENU="$ROOT/iso/grub/grub.cfg"
if [ "$EDITION" = "minimal" ]; then
    MENU="$ROOT/iso/grub/grub-minimal.cfg"
fi
cp -- "$MENU" "$WORK/isodir/boot/grub/grub.cfg"
VERSION="$(cat "$ROOT/VERSION" 2>/dev/null || echo 0.1.0)"
printf 'UnifyOS %s (%s edition) built %s\n' "$VERSION" "$EDITION" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    > "$WORK/isodir/.disk/info"

echo "[4/6] building boot loaders"
cat > "$WORK/embed.cfg" <<EOF
search --no-floppy --label --set=root $LABEL
if [ -z "\$root" ]; then
    search --no-floppy --file --set=root /boot/grub/grub.cfg
fi
if [ -z "\$root" ]; then
    echo "UnifyOS: cannot find the installation medium."
    sleep 10
    halt
fi
set prefix=(\$root)/boot/grub
configfile (\$root)/boot/grub/grub.cfg
EOF

GRUB_MODULES="part_gpt part_msdos iso9660 fat normal search search_label configfile all_video gfxterm font echo test linux"
"$GRUB_MKSTANDALONE" -O x86_64-efi --directory="$GRUB_LIB/x86_64-efi" \
    --locales="" --fonts="" \
    --output="$WORK/isodir/EFI/BOOT/BOOTX64.EFI" \
    --modules="$GRUB_MODULES" \
    "boot/grub/grub.cfg=$WORK/embed.cfg"

"$GRUB_MKIMAGE" -O i386-pc -d "$GRUB_LIB/i386-pc" -o "$WORK/core.img" \
    -p /boot/grub $GRUB_MODULES
cat "$GRUB_LIB/i386-pc/cdboot.img" "$WORK/core.img" > "$WORK/isodir/boot/grub/eltorito.img"

# Small FAT image that carries the UEFI loader; used as the El Torito EFI entry
# and as the hybrid GPT partition so the ISO also boots when dd'ed to a stick.
EFI_IMAGE="$WORK/isodir/boot/grub/efi.img"
dd if=/dev/zero of="$EFI_IMAGE" bs=1M count=8 status=none
"$MFORMAT" -i "$EFI_IMAGE" ::
"$MMD" -i "$EFI_IMAGE" ::/EFI ::/EFI/BOOT
"$MCOPY" -i "$EFI_IMAGE" "$WORK/isodir/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/BOOTX64.EFI

echo "[5/6] writing hybrid ISO"
rm -f "$OUTPUT"
"$XORRISO" -as mkisofs -o "$OUTPUT" -V "$LABEL" -J -joliet-long -R \
    -b boot/grub/eltorito.img -no-emul-boot -boot-load-size 4 -boot-info-table --grub2-boot-info \
    -eltorito-alt-boot -e boot/grub/efi.img -no-emul-boot -isohybrid-gpt-basdat \
    -isohybrid-mbr "$GRUB_LIB/i386-pc/boot_hybrid.img" \
    "$WORK/isodir"

if [ ! -s "$OUTPUT" ]; then
    echo "error: ISO was not produced" >&2
    exit 1
fi

echo "[6/6] checksums"
(
    cd "$(dirname "$OUTPUT")"
    sha256sum "$(basename "$OUTPUT")" > "$(basename "$OUTPUT").sha256"
)

if [ "$KEEP_WORK" -eq 0 ]; then
    rm -rf "$WORK"
fi

echo
echo "Built $OUTPUT"
ls -lh "$OUTPUT"
cat "$OUTPUT.sha256"
