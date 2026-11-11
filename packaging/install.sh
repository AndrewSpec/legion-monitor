#!/bin/sh
# Install a release bundle or a locally built legion-monitor for the current user.
#
#   ./packaging/install.sh    # from a checkout, after cargo build --release
#   ./install.sh              # from an unpacked release bundle
#
# Honours PREFIX (default: ~/.local). The binary comes from
# target/release/legion-monitor in a checkout, or from next to this script in an
# unpacked bundle; the icons and desktop entry are looked up the same way.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/.." && pwd)
prefix=${PREFIX:-$HOME/.local}

# A checkout keeps the binary in target/release and the assets in assets/ and
# packaging/, while an unpacked release bundle keeps them next to install.sh.
find_first() {
    for candidate in "$@"; do
        if [ -e "$candidate" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

binary=$(find_first "$root/target/release/legion-monitor" \
    "$script_dir/legion-monitor") || {
    echo "missing legion-monitor - build it with 'cargo build --release'" \
        "or unpack a release bundle" >&2
    exit 1
}
[ -x "$binary" ] || {
    echo "$binary is not executable" >&2
    exit 1
}

icon_svg=$(find_first "$root/assets/icon.svg" "$script_dir/icon.svg") || icon_svg=
icon_png=$(find_first "$root/assets/icon.png" "$script_dir/icon.png") || {
    echo "missing icon.png" >&2
    exit 1
}
desktop=$(find_first "$root/packaging/legion-monitor.desktop" \
    "$script_dir/legion-monitor.desktop") || {
    echo "missing legion-monitor.desktop" >&2
    exit 1
}

install -Dm755 "$binary" "$prefix/bin/legion-monitor"

# Multi-size icons when rsvg-convert is available; otherwise the 256 px PNG is
# still usable because every desktop environment scales it.
for size in 16 24 32 48 64 96 128 256 512; do
    dir="$prefix/share/icons/hicolor/${size}x${size}/apps"
    if [ -n "$icon_svg" ] && command -v rsvg-convert >/dev/null 2>&1; then
        install -d "$dir"
        rsvg-convert -w "$size" -h "$size" "$icon_svg" \
            -o "$dir/legion-monitor.png"
    elif [ "$size" -eq 256 ]; then
        install -Dm644 "$icon_png" "$dir/legion-monitor.png"
    fi
done

install -Dm644 "$desktop" "$prefix/share/applications/legion-monitor.desktop"

# Refresh desktop caches when the helpers exist; failures are not fatal.
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$prefix/share/icons/hicolor" >/dev/null 2>&1 || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
fi

if ! command -v ddcutil >/dev/null 2>&1; then
    echo "warning: ddcutil is not installed; legion-monitor needs it to reach" \
        "the monitor (openSUSE: sudo zypper install ddcutil," \
        "Debian/Ubuntu: sudo apt install ddcutil)" >&2
fi

echo "installed legion-monitor into $prefix"
echo "make sure $prefix/bin is in PATH and restart the application"
