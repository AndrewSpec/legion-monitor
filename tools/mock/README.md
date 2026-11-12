# Offline monitor mocks

The Overview monitor graphic depends on data that normally comes from the
physical monitor. These scripts replace `ddcutil` and `kscreen-doctor` with
canned answers so the GUI can be built, rendered and reviewed without hardware.
The canned monitor also reports a representative picture/color feature set
(brightness, contrast, gamma, sharpness, temperature, preset and RGB gains).

They implement only the calls this application makes; `setvcp` is accepted but
does not change the canned readback, so a render can show an active split but
cannot exercise a mode switch end to end. The values are plausible
test data, not measurements, and a successful render does not prove hardware
support.

## Scenarios

| `LEGION_MOCK_SCENARIO` | Panel | State |
|---|---|---|
| `single-16:9` | 1920 × 1080, unknown model (desktop fallback) | single input |
| `single-21:9` | Legion R34w-30, 3440 × 1440 | single input |
| `single-32:9` | Legion R45w-30, 5120 × 1440 | single input |
| `pbp-32:9` | Legion R45w-30, 5120 × 1440 | PBP 50/50, USB-C left, HDMI 1 right |
| `pbp-32:9-75` | Legion R45w-30, 5120 × 1440 | PBP with an unexpected 75/25 layout (`0xF5=0x0401`); the app repairs it to 50/50 |
| `pip-21:9` | Legion R34w-30, 3440 × 1440 | PiP, DisplayPort main, HDMI 2 inset |
| `true-split-32:9` | Legion R45w-30, 5120 × 1440 | Hardware True Split 50/50 over USB-C (`0xF2=0x0103`, `0xF7=1`) |
| `true-split-stale` | Legion R45w-30, 5120 × 1440 | True Split after the OSD: USB-C in `0x60`, stale `0xF5=0x0402` |

## Rendering a page

```bash
cargo build
tools/mock/render.sh single-32:9
tools/mock/render.sh pbp-32:9 gui-pbp.png
tools/mock/render.sh pbp-32:9 gui-multiview.png multi-view 1180x1370
```

`render.sh` starts Xvfb, puts `tools/mock` first on `PATH`, resizes the window
to 1180 × 1200 (or the optional fourth argument, `WIDTHxHEIGHT`) and captures
it with ImageMagick. The optional third argument is the page passed to
`--page` (default `overview`). Override the connector with
`LEGION_MOCK_CONNECTOR` if a test needs an HDMI link.

For the English documentation screenshots, point `XDG_CONFIG_HOME` at a
temporary directory with `{"language":"en"}` in
`legion-monitor/settings.json`.

## Running the application interactively against a mock

```bash
PATH="$PWD/tools/mock:$PATH" LEGION_MOCK_SCENARIO=pbp-32:9 \
    ./target/debug/legion-monitor --page overview
```

On a Wayland desktop, force the X11 backend for an Xvfb run
(`unset WAYLAND_DISPLAY; export WINIT_UNIX_BACKEND=x11`) and use software GL.
