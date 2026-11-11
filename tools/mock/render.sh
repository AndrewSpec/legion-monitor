#!/bin/sh
# Render the Overview page against a canned monitor under Xvfb.
#
#   tools/mock/render.sh [scenario] [output.png] [page]
#
# Scenarios: single-16:9, single-21:9, single-32:9, pbp-32:9, pip-21:9.
# Requires cargo build first, plus Xvfb, xdotool and ImageMagick's `import`.

set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../.." && pwd)
scenario=${1:-single-32:9}
out=${2:-$root/gui-mock-$(printf '%s' "$scenario" | tr ':' 'x').png}
page=${3:-overview}
bin="$root/target/debug/legion-monitor"

[ -x "$bin" ] || {
    echo "missing $bin — run: cargo build" >&2
    exit 1
}

rm -f "$out"

xvfb-run -a -s "-screen 0 1400x1300x24 +extension GLX +render -noreset" sh -c '
    unset WAYLAND_DISPLAY
    export WINIT_UNIX_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1
    export PATH="'"$root"'/tools/mock:$PATH"
    export LEGION_MOCK_SCENARIO="'"$scenario"'"

    "'"$bin"'" --page "'"$page"'" >/dev/null 2>&1 &
    pid=$!
    trap "kill $pid 2>/dev/null || true" EXIT

    win=
    i=0
    while [ "$i" -lt 80 ]; do
        win=$(xdotool search --name "Legion Monitor" 2>/dev/null | head -n 1 || true)
        [ -n "$win" ] && break
        i=$((i + 1))
        sleep 0.1
    done
    [ -n "$win" ] || { echo "window did not appear" >&2; exit 1; }

    sleep 1
    xdotool windowsize "$win" 1180 1200
    sleep 1
    import -window "$win" "'"$out"'"
'

echo "$out"
