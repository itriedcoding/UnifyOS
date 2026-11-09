#!/usr/bin/env bash
# Prove the live init cannot exit as PID 1.
#
# The kernel panics with "Attempted to kill init" the instant PID 1 exits, which
# is exactly what a Hyper-V generation-2 machine showed at 0.48 s: its DVD sits
# on the synthetic SCSI controller, the initramfs had no driver for it, the
# medium was never found, and the rescue path ended in a shell that exited on
# end of input.
#
# This runs the real iso/live/init in a private mount namespace with no live
# medium present and requires it to still be running when the timeout fires.
#
# Needs root for chroot and mount namespaces; CI runs it as root.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUSYBOX="${BUSYBOX:-$(command -v busybox || true)}"

if [ "$(id -u)" -ne 0 ]; then
    echo "skip: needs root for chroot and mount namespaces" >&2
    exit 0
fi
if [ -z "$BUSYBOX" ] || [ ! -x "$BUSYBOX" ]; then
    echo "error: busybox not found (install busybox-static)" >&2
    exit 1
fi

STAGE="$(mktemp -d)"
LOG="$(mktemp)"
trap 'rm -rf "$STAGE" "$LOG"' EXIT

mkdir -p "$STAGE"/{bin,dev,proc,sys,media,root}
cp "$BUSYBOX" "$STAGE/bin/busybox"
for applet in sh mount umount insmod switch_root sleep echo cat mkdir readlink \
              dmesg ls uname tail sed grep mknod setsid cttyhack; do
    ln -sfn busybox "$STAGE/bin/$applet"
done
install -m755 "$ROOT/iso/live/init" "$STAGE/init"
: > "$STAGE/modules.load"

echo "running iso/live/init without a live medium; it must never return"

# The init waits up to 15 s for a device node before it gives up, so the window
# has to be longer than that or the run ends in the wait instead of the failure.
WINDOW=25
set +e
timeout "$WINDOW" unshare -m --propagation private chroot "$STAGE" /init </dev/null >"$LOG" 2>&1
status=$?
set -e

echo "--- init output ---"
sed 's/^/  | /' "$LOG"

if [ "$status" -ne 124 ]; then
    echo "FAILED: init exited (status $status) instead of staying alive."
    echo "        The kernel would panic with 'Attempted to kill init' and the"
    echo "        machine would never reach a session."
    exit 1
fi
if ! grep -q "no live medium" "$LOG"; then
    echo "FAILED: init did not report the missing medium"
    exit 1
fi
if ! grep -q "rescue shell" "$LOG"; then
    echo "FAILED: init did not reach the rescue shell"
    exit 1
fi

echo "ok: the missing-medium path keeps PID 1 alive"
