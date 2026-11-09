#!/usr/bin/env bash
# Bootstrap a UnifyOS root filesystem from Debian bookworm packages.
#
# Two modes are supported:
#
#   root          the normal path: mmdebstrap (or debootstrap) runs with real
#                 privileges and installs the profile directly.
#   unprivileged  for CI containers without privileges: mmdebstrap's chrootless
#                 mode plus small `chown` shims (Debian maintainer scripts call
#                 chown to non-root groups, which cannot succeed without a
#                 multi-ID user namespace). File ownership is normalised to root
#                 by `mksquashfs -all-root` when the ISO is assembled, which is
#                 exactly what a live image needs.
#
# The script then stages the UnifyOS overlay (desktop configuration, themes,
# branding, systemd units, prebuilt binaries) into the new rootfs and runs the
# edition hook inside it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EDITION="desktop"
TARGET=""
MODE="auto"
MIRROR="${MIRROR:-http://deb.debian.org/debian}"
SUITE="${SUITE:-bookworm}"
ARCH="${ARCH:-amd64}"
INSECURE="${INSECURE:-0}"
# Path to a debian-archive-keyring.gpg to trust for the bootstrap. Non-Debian
# hosts (for example Ubuntu runners) ship a keyring that is often too old to
# verify the current archive, which makes apt reject every InRelease file.
KEYRING="${KEYRING:-}"

usage() {
    cat <<'EOF'
Usage: bootstrap-rootfs.sh [options]

  --edition NAME   desktop | minimal              (default: desktop)
  --target DIR     rootfs destination             (default: build/rootfs-<edition>)
  --mode MODE      auto | root | unprivileged     (default: auto)
  --mirror URL     Debian mirror
  --suite NAME     Debian suite                   (default: bookworm)
  --arch ARCH      architecture                   (default: amd64)
  --insecure       use [trusted=yes] sources (no keyring configured)
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --edition) EDITION="$2"; shift 2 ;;
        --target) TARGET="$2"; shift 2 ;;
        --mode) MODE="$2"; shift 2 ;;
        --mirror) MIRROR="$2"; shift 2 ;;
        --suite) SUITE="$2"; shift 2 ;;
        --arch) ARCH="$2"; shift 2 ;;
        --insecure) INSECURE=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

TARGET="${TARGET:-$ROOT/build/rootfs-$EDITION}"
PROFILE="$ROOT/iso/profiles/$EDITION.list"
[ -f "$PROFILE" ] || { echo "error: missing profile $PROFILE" >&2; exit 1; }

if [ "$MODE" = "auto" ]; then
    if [ "$(id -u)" -eq 0 ]; then MODE="root"; else MODE="unprivileged"; fi
fi
if [ "$MODE" = "unprivileged" ] && [ ! -x "$ROOT/tools/shim/chown" ]; then
    echo "error: unprivileged mode needs tools/shim/chown (run scripts/setup-build-tools.sh)" >&2
    exit 1
fi

mapfile -t PACKAGES < <(sed -e 's/#.*//' -e 's/[[:space:]]//g' "$PROFILE" | grep -v '^$' || true)
[ "${#PACKAGES[@]}" -gt 0 ] || { echo "error: profile $PROFILE is empty" >&2; exit 1; }
INCLUDE="$(IFS=,; echo "${PACKAGES[*]}")"

mkdir -p "$ROOT/build"
TRUSTED=""
[ "$INSECURE" = "1" ] && TRUSTED="[trusted=yes] "
cat > "$ROOT/build/sources.list" <<EOF
deb ${TRUSTED}${MIRROR} ${SUITE} main contrib non-free non-free-firmware
deb ${TRUSTED}${MIRROR} ${SUITE}-updates main contrib non-free non-free-firmware
EOF

echo "[1/4] bootstrapping $EDITION rootfs ($MODE) into $TARGET"
rm -rf "$TARGET"
mkdir -p "$TARGET"

COMMON_ARGS=(
    "--variant=minbase"
    "--arch=$ARCH"
    "--components=main,contrib,non-free,non-free-firmware"
    "--aptopt=APT::Install-Recommends \"false\";"
    "--include=$INCLUDE"
)

if [ -n "$KEYRING" ]; then
    [ -r "$KEYRING" ] || { echo "error: KEYRING is set but not readable: $KEYRING" >&2; exit 1; }
    COMMON_ARGS+=("--keyring=$KEYRING")
fi

if [ "$MODE" = "root" ]; then
    if command -v mmdebstrap >/dev/null 2>&1; then
        mmdebstrap "${COMMON_ARGS[@]}" "$SUITE" "$TARGET" "$ROOT/build/sources.list"
    elif command -v debootstrap >/dev/null 2>&1; then
        debootstrap --variant=minbase --arch="$ARCH" \
            --components=main,contrib,non-free,non-free-firmware \
            --include="$INCLUDE" "$SUITE" "$TARGET" "$MIRROR"
    else
        echo "error: neither mmdebstrap nor debootstrap is installed" >&2
        exit 1
    fi
else
    if ! command -v mmdebstrap >/dev/null 2>&1; then
        echo "error: unprivileged mode requires mmdebstrap" >&2
        exit 1
    fi
    SHIMS="$ROOT/tools/shim"
    PATH="$SHIMS:$PATH" mmdebstrap --mode=chrootless "${COMMON_ARGS[@]}" \
        --setup-hook="install -D -m0755 $SHIMS/chown \"\$1/usr/local/bin/chown\"; install -D -m0755 $SHIMS/chgrp \"\$1/usr/local/bin/chgrp\"" \
        "$SUITE" "$TARGET" "$ROOT/build/sources.list"
fi

echo "[2/4] staging UnifyOS overlay"
STAGE="$TARGET/opt/unify/overlay"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin"
for directory in desktop themes wallpapers branding; do
    [ -d "$ROOT/$directory" ] && cp -a "$ROOT/$directory" "$STAGE/"
done
for directory in units udev polkit bin; do
    [ -d "$ROOT/system/$directory" ] && cp -a "$ROOT/system/$directory" "$STAGE/"
done
if [ -d "$ROOT/installer" ]; then
    cp -a "$ROOT/installer" "$STAGE/installer"
fi
if [ -d "$ROOT/applications/builddir" ]; then
    cp -a "$ROOT/applications/builddir" "$STAGE/applications"
fi

# Prefer freshly built release binaries over anything tracked in system/bin.
for binary in unify-ctl unify-sysmond unify-pkg; do
    if [ -x "$ROOT/system/target/release/$binary" ]; then
        cp -a "$ROOT/system/target/release/$binary" "$STAGE/bin/$binary"
    fi
done

run_hook() {
    local hook="$1"
    install -D -m0755 "$hook" "$TARGET/tmp/unify-hook.sh"
    echo "      running $(basename "$hook")"
    if [ "$MODE" = "root" ]; then
        mkdir -p "$TARGET/dev" "$TARGET/proc" "$TARGET/sys"
        mount -t proc proc "$TARGET/proc"
        mount --bind /dev "$TARGET/dev"
        mount -t sysfs sysfs "$TARGET/sys"
        chroot "$TARGET" /bin/bash /tmp/unify-hook.sh
        umount -R "$TARGET/sys" "$TARGET/dev" "$TARGET/proc" || true
    else
        unshare -r -m --pid --fork bash -c "
            set -e
            mkdir -p '$TARGET/dev' '$TARGET/proc' '$TARGET/sys'
            mount -t proc proc '$TARGET/proc'
            mount --bind /dev '$TARGET/dev'
            mount -t sysfs sysfs '$TARGET/sys'
            chroot '$TARGET' /bin/bash /tmp/unify-hook.sh
            status=\$?
            umount -R '$TARGET/sys' '$TARGET/dev' '$TARGET/proc' || true
            exit \$status
        "
    fi
    rm -f "$TARGET/tmp/unify-hook.sh"
}

echo "[3/4] configuring the image"
run_hook "$ROOT/iso/hooks/common.chroot"
if [ "$EDITION" = "minimal" ]; then
    run_hook "$ROOT/iso/hooks/minimal.chroot"
else
    run_hook "$ROOT/iso/hooks/desktop.chroot"
fi

echo "[4/4] root filesystem ready: $TARGET ($(du -sh "$TARGET" | cut -f1))"
echo "      next: ./iso/make-iso.sh --rootfs $TARGET --edition $EDITION"
