#!/bin/sh
# Render a page against a canned monitor under Xvfb.
#
#   tools/mock/render.sh [scenario] [output.png] [page] [window size]
#
# Scenarios: single-16:9, single-21:9, single-32:9, pbp-32:9, pip-21:9.
# Window size defaults to 1180x1200; a taller page may need e.g. 1180x1370.
# Requires cargo build first, plus Xvfb, xdotool and ImageMagick's `import`.

set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../.." && pwd)
scenario=${1:-single-32:9}
out=${2:-$root/gui-mock-$(printf '%s' "$scenario" | tr ':' 'x').png}
page=${3:-overview}
size=${4:-1180x1200}
win_width=${size%x*}
win_height=${size#*x}
# The virtual screen must be larger than the window, or the capture is clipped.
screen_width=$((win_width + 120))
screen_height=$((win_height + 100))
if [ "$screen_width" -lt 1400 ]; then screen_width=1400; fi
if [ "$screen_height" -lt 1300 ]; then screen_height=1300; fi
bin="$root/target/debug/legion-monitor"

[ -x "$bin" ] || {
    echo "missing $bin — run: cargo build" >&2
    exit 1
}

rm -f "$out"

xvfb-run -a -s "-screen 0 ${screen_width}x${screen_height}x24 +extension GLX +render -noreset" sh -c '
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
    xdotool windowsize "$win" '"$win_width"' '"$win_height"'
    xdotool mousemove 5 5
    sleep 1
    import -window "$win" "'"$out"'"
'

echo "$out"
