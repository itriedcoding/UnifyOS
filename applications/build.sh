#!/usr/bin/env bash
# Build the UnifyOS GTK4 applications.
#
# On a normal system GTK4 development files come from the distribution and this
# script simply runs make. In the unprivileged build container the headers and
# libraries are extracted into build/gtkroot, so point pkg-config at that tree.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GTKROOT="${GTKROOT:-$ROOT/build/gtkroot}"

if [ -d "$GTKROOT/usr/lib/x86_64-linux-gnu/pkgconfig" ]; then
    export PKG_CONFIG_SYSROOT_DIR="$GTKROOT"
    export PKG_CONFIG_PATH="$GTKROOT/usr/lib/x86_64-linux-gnu/pkgconfig:$GTKROOT/usr/share/pkgconfig:$GTKROOT/usr/share/pkgconfig"
fi

CFLAGS_GTK="$(pkg-config --cflags gtk4)"
# cairo links freetype/fontconfig/pixman privately; name them so ld can resolve
# symbols when the sysroot's shared objects are used instead of a linker script.
LIBS_GTK="$(pkg-config --libs gtk4) -lfreetype -lfontconfig -lpixman-1 -lX11 -lXrender -lXext -lxcb -lpng16 -lxcb-render -lxcb-shm -Wl,--allow-shlib-undefined"

if [ -z "$CFLAGS_GTK" ]; then
    echo "error: pkg-config cannot find gtk4 (looked in $GTKROOT)" >&2
    exit 1
fi

exec make -C "$ROOT/applications" GTK_CFLAGS="$CFLAGS_GTK" GTK_LIBS="$LIBS_GTK" "$@"
