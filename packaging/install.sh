#!/bin/sh
# Install a locally built legion-monitor for the current user.
#
#   ./packaging/install.sh
#
# Honours PREFIX (default: ~/.local) and expects target/release/legion-monitor.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
prefix=${PREFIX:-$HOME/.local}
binary=$root/target/release/legion-monitor

if [ ! -x "$binary" ]; then
    echo "missing $binary - run 'cargo build --release' first" >&2
    exit 1
fi

install -Dm755 "$binary" "$prefix/bin/legion-monitor"

# Multi-size icons; without rsvg-convert the 256 px PNG is still usable.
for size in 16 24 32 48 64 96 128 256 512; do
    dir="$prefix/share/icons/hicolor/${size}x${size}/apps"
    install -d "$dir"
    if command -v rsvg-convert >/dev/null 2>&1; then
        rsvg-convert -w "$size" -h "$size" "$root/assets/icon.svg" \
            -o "$dir/legion-monitor.png"
    elif [ "$size" -eq 256 ]; then
        install -Dm644 "$root/assets/icon.png" "$dir/legion-monitor.png"
    fi
done

install -Dm644 "$root/packaging/legion-monitor.desktop" \
    "$prefix/share/applications/legion-monitor.desktop"

# Refresh desktop caches when the helpers exist; failures are not fatal.
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$prefix/share/icons/hicolor" >/dev/null 2>&1 || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
fi

echo "installed legion-monitor into $prefix"
echo "make sure $prefix/bin is in PATH and restart the application"
