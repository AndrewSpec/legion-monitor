# legion-monitor

[![CI](https://github.com/AndrewSpec/legion-monitor/actions/workflows/ci.yml/badge.svg)](https://github.com/AndrewSpec/legion-monitor/actions/workflows/ci.yml)
[![License: GPL-3.0-only](https://img.shields.io/badge/license-GPL--3.0--only-blue.svg)](LICENSE)

<img src="assets/icon.png" width="88" align="right" alt="Legion Monitor icon">

Control your **Lenovo Legion R45w-30** on Linux over DDC/CI without using its
physical buttons. The Rust GUI (egui/eframe) exposes brightness, contrast, color
and input controls, plus **experimental Multi-view** controls (PiP/PbP and window
layouts). All monitor communication goes through `ddcutil`.

**Hardware support is partly verified.** The direct two-source PBP write and all
input switches were confirmed on the physical R45w-30 on 2026-11-11. The safe
single-mode exit path described below is implemented but has not been exercised
on hardware yet. The mapping of all eight layout previews and the PiP geometry
still need a check on the monitor. Recent hardware tests also found ignored
brightness writes; see
[Local hardware test results](#local-hardware-test-results-2026-11-10).

![Legion Monitor interface](docs/screenshot.png)

<sub>Screenshot taken without a connected monitor: the interface remains available in offline mode.</sub>

## Requirements

- Linux with `ddcutil` (openSUSE: `sudo zypper install ddcutil`;
  Debian/Ubuntu: `sudo apt install ddcutil`).
- Access to `/dev/i2c-*` as a regular user. If your distribution uses the `i2c`
  group to grant device access, add your user and log out and back in:

```bash
sudo usermod -aG i2c "$USER"
```

Check access and monitor detection with `ddcutil detect`. Group membership alone
will not help if the device nodes or the required kernel module are missing.

- To build: Rust with Cargo and Wayland/X11/OpenGL development libraries.
  On openSUSE:
  `sudo zypper install rust cargo libxkbcommon-devel wayland-devel Mesa-libGL-devel`.
  On Debian/Ubuntu:
  `sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev`.

## Build and install

```bash
cargo build --release

./target/release/legion-monitor              # GUI
./target/release/legion-monitor --dump       # register state without the GUI
./target/release/legion-monitor --page pip    # open a specific page
```

`packaging/install.sh` installs the binary, icons and desktop entry under
`~/.local` (override this with `PREFIX`), then refreshes the icon cache and
KSycoca. Icons use hicolor sizes from 16 to 512 px when `rsvg-convert` is
available, or 256 px otherwise.

```bash
./packaging/install.sh
```

Make sure `~/.local/bin` is in `PATH`: the desktop entry launches
`legion-monitor` by name. Restart the application after installation so that the
taskbar picks up its icon.

Accepted `--page` values include `przeglad`, `obraz`, `kolor`, `zrodla`,
`multiview` (`pip`, `pbp`), `profile` and `zaawansowane`.

## Icon

The icon depicts a monitor split into two views, using the interface colors
(`#5b8cff` on `#0d1015`). Its vector source is `assets/icon.svg`.
The application window uses `assets/icon.rgba`: headerless RGBA8 data embedded
with `include_bytes!`, avoiding a PNG decoder or additional dependencies.
After changing the artwork, regenerate the assets:

```bash
rsvg-convert -w 256 -h 256 assets/icon.svg -o assets/icon.png
tools/png_to_rgba.py assets/icon.png assets/icon.rgba
```

For desktop integration, `packaging/install.sh` installs hicolor icons at
16–512 px, and the desktop entry uses `Icon=legion-monitor`.

**Wayland:** `with_icon` only applies on X11, Windows and macOS. On Wayland, the
taskbar obtains the icon from the desktop entry matching the window's `app_id`
(`legion-monitor`). The entry must therefore be installed in the applications
directory; a shortcut on the desktop alone is not enough. The installer handles
this and refreshes KSycoca. To do it manually:

```bash
install -Dm644 packaging/legion-monitor.desktop \
  ~/.local/share/applications/legion-monitor.desktop
kbuildsycoca6 --noincremental   # KDE; restart the application afterward
```

Without this integration, KDE may show a fallback “W” icon for the Wayland window.

## Interface

Navigation is on the left, with settings grouped into cards. Normal use does not
require reading VCP codes. Feature lists come from the monitor's reported
capabilities (`ddcutil capabilities`). Enumerated features use buttons, binary
0/1 features use switches, and other adjustable values use sliders.

An advertised capability does not guarantee that writes work, especially for
unknown manufacturer-specific features. Features the monitor does not report
are not shown at all: the rows are built from the values the monitor actually
returned, so an unsupported feature leaves no trace in the interface.

| Page | Contents |
|---|---|
| Overview | Display mode, active input, brightness, contrast and A/B configuration saving |
| Picture | Brightness, contrast, sharpness, saturation, hue, gamma and black level — only the features the monitor reports |
| Color | Color temperature, presets and RGB gains |
| Inputs | DisplayPort, HDMI 1, HDMI 2 and USB-C; the reported active input is highlighted |
| Multi-view (experimental) | Mode selection, eight layout codes and the source of each window; PBP writes both sources in one `0x60` pair, without a swap. Leaving a split checks for a second screen and, when there is none, keeps this computer’s source visible; geometry and PiP mapping remain unverified |
| Profiles | A/B configurations and any number of named profiles |
| Advanced | Volume, mute, power mode, manufacturer-specific features and reset |

Keyboard shortcuts: **F1** applies configuration A, **F2** applies configuration B,
and **F5** refreshes the state.

## Languages

The current interface supports eight languages: **Polish, English, Spanish,
French, Italian, Portuguese, Dutch and Czech**. Translation coverage does not
imply that every monitor feature has been verified on hardware.

The language picker is in the top bar. The default follows the system locale
(`LC_ALL`, `LC_MESSAGES`, `LANG`); a manual selection is saved in:

```text
~/.config/legion-monitor/settings.json     # {"language":"en"}
```

Without a saved selection, the application uses the system language.
To add a language, create a file in `src/i18n/` with an `&[(Key, &str)]` table,
then register it in `src/i18n/mod.rs`: the module, `Lang`, `LANGS`, `Lang::code`,
`strings()` and the test tables. Missing keys fall back to English, so an
incomplete translation does not break the interface.

The `every_language_has_the_same_keys` and `tables_have_no_duplicate_keys` tests
check key coverage, duplicates and `{}` placeholder counts against English.
Source comments are written in English, and interface text belongs in `i18n`.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

CI (`.github/workflows/ci.yml`) runs these checks and a release build on Ubuntu.
The default tests do not require a monitor.
`worker::tests::live_reversible_picture_and_profile_roundtrip` is ignored by
default and requires `LEGION_LIVE_TEST=1`, `LEGION_LIVE_SERIAL=<serial>` for
the identified test monitor and permission to access `/dev/i2c-*`.

## Profiles

Configurations A and B are two quick-access slots, available through buttons
and F1/F2. You can also save any number of named profiles. They are stored in:

```text
~/.config/legion-monitor/profiles.json
```

Example format:

```json
{
  "version": 1,
  "slots": { "a": null, "b": null },
  "profiles": [
    {
      "id": "p1",
      "name": "Example picture settings",
      "values": { "10": 80, "12": 70 }
    }
  ]
}
```

`values` maps hexadecimal VCP codes to numeric values. Saving and applying a
profile share an allowlist that excludes reset, status, power and unknown
manufacturer-specific registers. Only allowed settings present in the current
readback are saved. Values loaded from existing profile files are filtered too.

Applying a profile sends a sequence of writes with short pauses because mode
changes may interrupt the EDID/DDC connection. The color preset is restored
after temperature/RGB settings, since those writes can change the preset.
This ordering still needs a hardware roundtrip: some preset/RGB combinations
may not coexist. Multi-view settings are written in the order `0xF5`, then
`0xF2` (legacy exchange), with the input switch last. A PBP source change writes
`0xA5 = 0x0200`, an optional `0xF5` layout repair and then both sources in one
`0x60` write. Leaving a split uses a different sequence, because closing PBP can
leave the monitor on either half:

1. the application determines which monitor input this computer is using
   (the single-mode `0x60` value, a connector match in the active PBP pair, or
   the user’s choice);
2. if an independent second screen is connected, it only writes `0xA5 = 0` —
   the desktop can always be recovered on that screen;
3. without a second screen it first writes `0x60 = (source << 8) | source` while
   PBP is still active, so both halves show this computer, then closes the split
   with `0xA5 = 0`, and finally writes the single input `0x60 = source`.

The topology check uses `kscreen-doctor -j`, falling back to `xrandr --query`;
if neither can be read, the application treats the topology as unknown and asks
for the source instead of guessing. Success requires matching readback; a
successful command exit alone is not sufficient.

There is no guarantee of atomic rollback on failure: some settings may already
have changed. A/B highlighting reflects a match between the entire nonempty
replay plan and the latest monitor readback, not the last button clicked.
Slots are not marked as matching while an operation is running or when required
registers are missing from the readback.

## Protocol findings

These findings come from static analysis of Lenovo's Windows background service
(`LenovoDCCBackGroundService.exe` from `LenovoDCCServiceInstaller.msi`) and its
PIP/PBP web plugin, the `supportedmodels.json` shipped with Lenovo Display
Control Center 3.0.32161 (`vcpCodesDef`), and hardware observations. Accepting a
`0xF2`/`0xF5` write and reading back its value does not prove that the physical
picture matches a preview, that all eight geometries are mapped correctly, or
that PiP/PbP selection works as intended.

| Feature | VCP | Evidence / direction | Notes |
|---|---|---|---|
| Multi-view control | `0xF2` | Write and readback observed | Lenovo calls it *PIP/PBP Swap*: `0` disables the PIP/PBP window, `1` swaps the sources, `3` leaves full-screen mode |
| Layout selection | `0xF5` | Write and readback observed | Lenovo calls it *PBP Control*: low byte = position (`1` right, `2` left, `3` top, `4` bottom), high byte = ratio (`1` 1:1, `2` 2:1, `3` 1:2, `4` 3:1). On the R45w-30 only `0x0101` (50/50) and `0x0401` (70/30) change the picture, the `…02` values are rejected and `0x0201`/`0x0301` are ignored |
| Layout status | `0xF4` | Read in the examined path | Lenovo calls it *PIP Size* (`0` min … `0x64` max); the R45w-30 uses it differently and reports `0x0201` for every active split |
| Window selection | `0xA5` | Write and readback observed | Lenovo calls it *Window Select*: `0x0000` closes PIP/PBP, `0x0100` selects the PIP window, `0x0200` the PBP window. The monitor never reads the selection back |
| Window inputs | `0xF0` | Read in the examined path | Lenovo calls it *Audio Source*; on the R45w-30 it reports the two window sources (low byte = left window, high byte = right) but is unreadable most of the time (`0xFFFF`) |
| Input source | `0x60` | Read/write control | `0x0f` DP, `0x11` HDMI 1, `0x12` HDMI 2, `0x31` USB-C. Outside a split it takes one source byte. In PBP Lenovo writes the whole pair as `(right << 8) | left`; that is the direct way to set the right half, with no `0xA5` selector and no `0xF2` exchange. `0xA5 = 0x0200` was verified on the R45w-30; `0xA5 = 0x0100` in PBP froze the monitor's OSD |
| Brightness / contrast | `0x10` / `0x12` | Standard DDC controls | Reads work in local tests; the recent brightness-change test failed, as described below |

Lenovo's `SetPIPPBPState` opens PBP with `0xA5 = 0x0200`; `SetPBPMode` writes the
`F5` layout; `SetPIPPBPConfig` reads `F5` and writes `0x60` as the two-source pair.
The examined Multi-view path does not write `F4` directly. This does not prove
that other paths or versions never write it.

After an `F2`/`F5`/`0xA5` write, the monitor may disappear from the bus for
several seconds (`EDID has changed`). Reads are therefore retried, and writes use
`--noverify`. This option does not replace later readback or visual verification
on the physical monitor.

## Local hardware test results (2026-11-10/11)

The tested R45w-30 reported brightness `100`, contrast `87` and USB-C input
(`0x31`); these values also appeared in the GUI. The read-path repair includes
`--terse` output, short batches of VCP codes and removal of the worker's
48-feature limit.

**Earlier failed experiment (2026-11-10):** a brightness change `100 → 95`
returned exit code `0` from `ddcutil setvcp --noverify`, but delayed readback
still returned `100`. Temporarily disabling HDR in **KDE** produced the same
result; KDE HDR was restored afterward. This experiment did not establish the
cause of the ignored write.

**Later observations reported by a separate hardware-test session:** with
`HDR: Auto` in the **monitor's OSD**, brightness/contrast writes reportedly
failed verification; after manually selecting `HDR: Off` there, the session
reported matching readbacks for brightness `100 → 89`, contrast `87 → 76`, RGB
gains and the color preset. KDE HDR and the monitor's OSD HDR setting are
different controls. These later observations suggest an OSD-dependent write
restriction, but do not by themselves establish a universal cause for every
earlier ignored write. The report lists OSD HDR restoration to `Auto` as a
manual follow-up; it has not been rechecked in this repair pass.

Other confirmed findings:

- Sharpness (`0x87`) is shown in the raw DDC scale while the OSD shows 0-100 in
  steps of 20 (`4 → 80`, `5 → 100`). The monitor reports a maximum of `4` but
  uses `5` for OSD 100, and writing `5` always fails — that value can only be
  restored from the OSD.
- Gamma (`0x72`) is displayed as a raw number (`35840` for `+0.2`) and its
  slider (`0..40960`) cannot reliably hit the monitor's discrete presets: a
  one-pixel error produced `35500`, which the monitor rejected.
- The color temperature row offers four fixed values while the monitor uses a
  `0..63` scale, so the current value is never highlighted and a click writes an
  unrelated value.
- In the tested `1f7936b` build, the inputs page repeated entries and labelled
  USB-C as "Unrecognized value", because the capability string lists
  `0F 11 12 31 FF 0F 11 12 31`. The current working-tree parser deduplicates
  inputs and uses known connector names; this is a software fix, not a new
  hardware verification.
- Switching the input from USB-C to HDMI 1 and back worked; the monitor kept
  answering over DDC while the picture was gone.
- Multi-view: of the eight layout previews only two do anything on this monitor.
  `0x0101` splits the picture 50/50 (confirmed on screen) and `0x0401` splits it 75/25.
  The tiles show the raw register codes, so nothing misleading is drawn, but the
  names ("four windows") are guesses from the Lenovo Windows service, not from a
  measurement. The four "mirrored" values are rejected by the monitor and two more
  are ignored, and the "picture in picture" button only writes `0xF2` and creates no
  small window. `0xF4` always reports `0x0201` while a split is active, so it does not
  describe the layout, and entering a split switches the desktop output from
  5120×1440 to 2560×1440.
- In PBP both window sources are written at once: `0x60 = (right << 8) | left`.
  The low byte is the left/primary window and the high byte the right/secondary
  one — the same encoding as `0xF0`, and the same write Lenovo's
  `SetPIPPBPConfig` performs. No `0xA5` window selector and no `0xF2` exchange are
  needed, so the right half is reached directly and the monitor's OSD is not put
  into the state that froze it. The application now builds exactly this pair, so
  the old "write the left half, then swap" workaround is gone.
- `0xA5 = 0x0200` opens the PBP window and `0x0100` selects the PIP window; mixing
  them up is what froze the OSD on the R45w-30. Outside a split `0x60` still takes
  a single source byte.
- `0xF0` reports the sources of both windows (low byte = left, high byte = right)
  but is unreadable most of the time (`0xFFFF`). The application falls back to the
  pair read from `0x60`.
- On 2026-11-11 the direct PBP pair write was exercised through the GUI on the
  R45w-30: PBP was enabled, the left and right sources were selected, and all
  inputs switched correctly. The old swap path was not used and the OSD did not
  freeze.
- The single-mode exit safety logic was added afterwards and is covered by unit
  tests and an offline UI run only. It has not been sent to a monitor yet; the
  hardware check is deliberately deferred until the laptop is connected again.
  In particular, `0x60 = (source << 8) | source` while PBP is active is the
  step that still needs confirmation on the R45w-30.

The full field-by-field matrix and the remaining gaps are kept as local
hardware notes outside the repository. A successful build and passing tests
without a monitor do not establish
hardware support. The PBP source path has since been exercised on the monitor;
the layout geometry and PiP mapping remain unverified.

## Offline behavior

When no monitor is connected, the application shows “Offline” and keeps all
sections visible with empty values as an interface preview, not a claim of
monitor support. Profiles remain accessible, but applying settings requires a
connection and readback. “Search again” reruns monitor detection.

## Code structure

| File | Responsibility |
|---|---|
| `src/main.rs` | Entry point, `--dump`, `--page` |
| `src/ddc.rs` | `ddcutil` calls and `detect`/`capabilities`/`getvcp` parsing |
| `src/catalog.rs` | Feature catalog (VCP codes → `Key`), sections, layouts and modes |
| `src/i18n/mod.rs` | `Lang`, `Key`, `Strings` and locale-based language selection |
| `src/i18n/<lang>.rs` | Translation tables (`ENTRIES`) |
| `src/settings.rs` | Selected-language persistence in `settings.json` |
| `src/topology.rs` | Desktop display scan (KScreen/XRandR) and monitor-link inference |
| `src/worker.rs` | Worker thread, commands, events and read retries |
| `src/profiles.rs` | Profile JSON persistence and configuration directory |
| `src/ui/mod.rs` | Application state (`App`), update loop, shortcuts and worker communication |
| `src/ui/theme.rs` | Palette, styling and navigation icons |
| `src/ui/widgets.rs` | Cards, chips, segmented controls and layout tiles |
| `src/ui/rows.rs` | Feature rows: sliders, switches, choices and placeholders |
| `src/ui/chrome.rs` | Top bar, sidebar, offline banner and language picker |
| `src/ui/pages/*.rs` | One module per page |
| `assets/icon.svg` | Vector source for the application icon |
| `assets/icon.png` | 256×256 desktop icon |
| `assets/icon.rgba` | Raw RGBA data embedded in the window (`include_bytes!`) |
| `packaging/legion-monitor.desktop` | Desktop entry, icon, `StartupWMClass` and `app_id` |
| `packaging/install.sh` | Binary, icon and desktop entry installation under `~/.local` |
| `tools/png_to_rgba.py` | PNG → RGBA conversion after icon changes |
| `docs/screenshot.png` | Interface screenshot used in this README |
| `.github/workflows/ci.yml` | Formatting, Clippy, tests and build in CI |
| `LICENSE` | GPL-3.0-only license |

## License

This project is licensed under **GPL-3.0-only**. See [LICENSE](LICENSE) for the
full license text.
