#!/usr/bin/env bash
# Boot-test the UnifyOS ISO under both firmware types.
#
# Run this on a Linux host with QEMU, SeaBIOS and OVMF installed:
#
#   sudo apt install qemu-system-x86 ovmf seabios
#   ./scripts/qemu-boot-test.sh out/UnifyOS-0.1.0-minimal-amd64.iso
#
# Each case boots the image headless, captures the serial console to a log in
# build/boottest/, and checks for the markers the live init prints. A missing
# marker is a failure: the script exits non-zero and prints the log tail, so it
# can gate CI instead of being advisory.
#
# Environment: QEMU, QEMU_MEMORY (default 1024), BOOT_TIMEOUT (default 240),
# OVMF_CODE, OVMF_VARS.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ISO="${1:-}"
QEMU="${QEMU:-qemu-system-x86_64}"
MEMORY="${QEMU_MEMORY:-1024}"
TIMEOUT="${BOOT_TIMEOUT:-240}"
LOGDIR="$ROOT/build/boottest"
ACCEL="-accel tcg"
[ -w /dev/kvm ] && ACCEL="-accel kvm"

if [ -z "$ISO" ] || [ ! -f "$ISO" ]; then
    echo "usage: $0 path/to/image.iso" >&2
    exit 2
fi
if ! command -v "$QEMU" >/dev/null 2>&1; then
    echo "error: $QEMU not found; install qemu-system-x86" >&2
    exit 2
fi

OVMF_CODE="${OVMF_CODE:-}"
OVMF_VARS="${OVMF_VARS:-}"
if [ -z "$OVMF_CODE" ]; then
    for candidate in /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/OVMF/OVMF_CODE.fd \
                     /usr/share/ovmf/OVMF.fd /usr/share/qemu/OVMF.fd; do
        [ -f "$candidate" ] && OVMF_CODE="$candidate" && break
    done
fi
if [ -z "$OVMF_VARS" ]; then
    for candidate in /usr/share/OVMF/OVMF_VARS_4M.fd /usr/share/OVMF/OVMF_VARS.fd; do
        [ -f "$candidate" ] && OVMF_VARS="$candidate" && break
    done
fi

mkdir -p "$LOGDIR"
failures=0

check_log() {
    local name="$1" log="$2"
    local required=(
        "unify-live: live medium found"
        "Live session ready"
    )
    local missing=0
    for marker in "${required[@]}"; do
        if ! grep -qF -- "$marker" "$log"; then
            echo "  MISSING: $marker"
            missing=1
        fi
    done
    if [ "$missing" -ne 0 ]; then
        echo "  --- last 40 lines of $log ---"
        tail -40 "$log" | sed 's/^/  | /'
        failures=$((failures + 1))
        return 1
    fi
    echo "  ok"
}

boot_bios() {
    local log="$LOGDIR/bios.log"
    echo "[bios] booting $ISO"
    timeout "$TIMEOUT" "$QEMU" \
        -machine q35 -m "$MEMORY" ${ACCEL} \
        -drive file="$ISO",media=cdrom,readonly=on \
        -boot d -no-reboot -nographic -monitor none \
        -serial "file:$log" >/dev/null 2>&1 || true
    check_log bios "$log"
}

boot_uefi() {
    if [ -z "$OVMF_CODE" ] || [ -z "$OVMF_VARS" ]; then
        echo "[uefi] SKIPPED: OVMF firmware not found (install ovmf)"
        failures=$((failures + 1))
        return 1
    fi

    local log="$LOGDIR/uefi.log"
    local vars="$LOGDIR/OVMF_VARS.fd"
    cp "$OVMF_VARS" "$vars"

    echo "[uefi] booting $ISO with $OVMF_CODE"
    timeout "$TIMEOUT" "$QEMU" \
        -machine q35 -m "$MEMORY" ${ACCEL} \
        -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
        -drive if=pflash,format=raw,file="$vars" \
        -drive file="$ISO",media=cdrom,readonly=on \
        -boot d -no-reboot -nographic -monitor none \
        -serial "file:$log" >/dev/null 2>&1 || true
    check_log uefi "$log"
}

echo "UnifyOS boot test: $ISO"
boot_bios || true
boot_uefi || true

echo
if [ "$failures" -ne 0 ]; then
    echo "FAILED: $failures case(s) did not reach the live session"
    echo "logs: $LOGDIR"
    exit 1
fi
echo "Both firmware types booted to the live session."
