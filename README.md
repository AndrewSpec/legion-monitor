# legion-monitor

[![CI](https://github.com/AndrewSpec/legion-monitor/actions/workflows/ci.yml/badge.svg)](https://github.com/AndrewSpec/legion-monitor/actions/workflows/ci.yml)
[![License: GPL-3.0-only](https://img.shields.io/badge/license-GPL--3.0--only-blue.svg)](LICENSE)

<img src="assets/icon.png" width="88" align="right" alt="Legion Monitor icon">

Control your **Lenovo Legion R45w-30** on Linux over DDC/CI without using its
physical buttons. The Rust GUI (egui/eframe) exposes brightness, contrast, color
and input controls, plus **experimental Multi-view** controls (PiP/PbP and window
layouts). All monitor communication goes through `ddcutil`.

**Hardware support is not fully verified.** The mapping of all eight layout
previews, PiP/PbP modes and window swapping remains unconfirmed. Recent hardware
tests also found ignored brightness writes; see [Local hardware test results](#local-hardware-test-results-2026-11-10).

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
appear as placeholder rows: they show the available interface sections, not
proof of device support.

| Page | Contents |
|---|---|
| Overview | Display mode, active input, brightness, contrast and A/B configuration saving |
| Picture | Brightness, contrast, sharpness, saturation, hue, gamma and black level |
| Color | Color temperature, presets and RGB gains |
| Inputs | DisplayPort, HDMI 1, HDMI 2 and USB-C; the reported active input is highlighted |
| Multi-view (experimental) | Mode selection and eight layout previews; geometry, PiP/PbP mapping and window swapping remain unverified |
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
default and requires `LEGION_LIVE_TEST=1`, the identified test monitor and
permission to access `/dev/i2c-*`.

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
changes may interrupt the EDID/DDC connection. Multi-view settings are written
in the order `0xF5`, then `0xF2`, with the input switch last. Success requires
matching readback; a successful command exit alone is not sufficient.

There is no guarantee of atomic rollback on failure: some settings may already
have changed. A/B highlighting reflects a match between the entire nonempty
replay plan and the latest monitor readback, not the last button clicked.
Slots are not marked as matching while an operation is running or when required
registers are missing from the readback.

## Protocol findings

These findings come from static analysis of Lenovo's Windows service
(`LDCCService.exe` from `LenovoDCCServiceInstaller.msi`) and limited earlier
hardware observations. Accepting a `0xF2`/`0xF5` write and reading back its value
does not prove that the physical picture matches a preview, that all eight
geometries are mapped correctly, or that PiP/PbP selection or window swapping
works as intended.

| Feature | VCP | Evidence / direction | Notes |
|---|---|---|---|
| Multi-view control | `0xF2` | Write and readback observed | Analysis suggests `1` / `0` for enable / disable; accepting a value does not confirm mode mapping |
| Layout selection | `0xF5` | Write and readback observed | Candidate values `0x0101`…`0x0402` and `0`; all eight physical geometries remain unverified |
| Layout status | `0xF4` | Read in the examined path | Decoding `0x01xx` as PiP, `0x02xx` as PbP and the low byte as a variant comes from Windows code analysis, not verified hardware mapping |
| Window inputs | `0xF0` | Unreliable readback | Byte meanings and left/right or main/secondary assignments remain unverified |
| Input source | `0x60` | Read/write control | `0x0f` DP, `0x11` HDMI 1, `0x12` HDMI 2, `0x31` USB-C |
| Brightness / contrast | `0x10` / `0x12` | Standard DDC controls | Reads work in local tests; the recent brightness-change test failed, as described below |

The examined Multi-view path writes `F2` and `F5` and reads `F4` for status; no
direct writes to `F4` were found there. This does not prove that other paths or
versions never write `F4`.

After an `F2`/`F5` write, the monitor may disappear from the bus for several
seconds (`EDID has changed`). Reads are therefore retried, and writes use
`--noverify`. This option does not replace later readback or visual verification
on the physical monitor.

## Local hardware test results (2026-11-10)

The tested R45w-30 reported brightness `100`, contrast `87` and USB-C input
(`0x31`); these values also appeared in the GUI. The read-path repair includes
`--terse` output, short batches of VCP codes and removal of the worker's
48-feature limit.

A brightness change from `100 → 95` failed: `ddcutil setvcp` exited with code `0`,
but subsequent readback still returned `100`. Temporarily disabling HDR produced
the same result; HDR was restored afterward. **The cause of ignored writes
remains unknown.** The application reports a readback mismatch instead of
success.

A GUI test with isolated A/B profiles ended with a readback of `100 / 87`; this
is not evidence of successful settings switching. Full PiP/PbP operation,
physical layout geometry and window swapping remain unverified. A successful
build and passing tests without a monitor do not establish hardware support.

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
