#!/usr/bin/env bash
# Render the real monitor state on a private Xvfb server.
#
# ddcutil needs /dev/i2c-*, which the DSH sandbox does not expose, so run this
# through the user manager, for example:
#
#   systemd-run --user --wait --pipe -q --setenv=HOME=/home/andrew \
#       --working-directory="$PWD" "$PWD/tools/render-live.sh" [output.png]
#
# The X11 backend is forced because the transient unit inherits the Wayland
# session, and a Wayland window would not appear on the private Xvfb screen.
set -u
cd "$(dirname "$0")/.." || exit 1
out=${1:-gui-live.png}
page=${2:-overview}
D=":98"
Xvfb "$D" -screen 0 1180x1000x24 -ac +extension GLX +render -noreset \
    >/tmp/legion-live-xvfb.log 2>&1 &
xp=$!
ap=
cleanup() {
    [ -n "$ap" ] && kill "$ap" 2>/dev/null
    kill "$xp" 2>/dev/null
}
trap cleanup EXIT
sleep 1
DISPLAY="$D" WAYLAND_DISPLAY= XDG_SESSION_TYPE=x11 WINIT_UNIX_BACKEND=x11 \
    LIBGL_ALWAYS_SOFTWARE=1 ./target/debug/legion-monitor --page "$page" \
    >/tmp/legion-live-app.log 2>&1 &
ap=$!
sleep 8
win=$(DISPLAY="$D" xdotool search --name "Legion Monitor" 2>/dev/null | head -1)
if [ -z "$win" ]; then
    echo "Legion Monitor window did not appear on $D" >&2
    cat /tmp/legion-live-app.log >&2
    exit 1
fi
DISPLAY="$D" import -window "$win" "$PWD/$out"
echo "wrote $PWD/$out"
