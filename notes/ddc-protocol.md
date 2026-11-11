# DDC/CI notes: Legion R45w-30

Technical notes from developing the Multi-view (True Split / PIP) and Rescue
features. They describe the monitor's DDC/CI behaviour as observed on the test
machine, not a vendor specification. No host names, addresses, serial numbers
or user paths are recorded here.

## Environment

- Monitor: Lenovo Legion R45w-30, 5120x1440, connected over DisplayPort.
- DDC/CI through `ddcutil` 2.2.7, one I2C bus per active DP/eDP connector.
- The monitor exposes a normal VCP capability list; some values are write-only
  or report stale data. Readback is evidence, not proof that the panel changed.
- `0xA5` writes can return `EIO(-5)` while the monitor still applies them.
  Treat the error as inconclusive and verify with `0xF4` and the rectangle
  registers.
- `0x60` reads back `Invalid value` for known inputs (for example `0x31` for
  USB-C). That is a display quirk, not a write failure.

## Registers used

| Code | Meaning | Notes |
| --- | --- | --- |
| `0x10` | Brightness | 0..100 |
| `0x12` | Contrast | 0..100 |
| `0x60` | Input source | Single byte = primary input. Pair = `(right << 8) \| left`; low byte is the left/primary source. |
| `0x95` | PIP rectangle left | Pixels, not percent |
| `0x96` | PIP rectangle top | Pixels |
| `0x97` | PIP rectangle right | Pixels |
| `0x98` | PIP rectangle bottom | Pixels |
| `0xA4` | PIP/PBP window mask | Reads as unsupported/all-zero. `0xFFFF` was rejected by `ddcutil`; the app does not write it. |
| `0xA5` | Window select | `0` close/full, `0x0100` PIP window, `0x0200` PBP window. Reads back as 0 even after it takes effect. |
| `0xD6` | Power mode | `1` on, `4` off; a DDC power cycle did not reset mode registers. |
| `0xF0` | Audio/source pair | Low byte left, high byte right on models that report it. |
| `0xF2` | PIP/PBP state | Low byte: `1` = PIP, `3` = True Split/PBP. High byte is a status flag and is not used for classification. |
| `0xF4` | PIP/PBP mode | `0x0200` single picture, `0x0201` any active split. It cannot distinguish PIP from PBP. |
| `0xF5` | True Split layout | `0x0101` = 50/50, `0x0401` = about 70/30. Other low-byte/ratio combinations are rejected or ignored. |

The monitor reports `0xF5 = 0x0402` after leaving a split. That value is stale;
do not trust it as the current layout. `0xF4` also stays at `0x0201` in any split
and a zero PIP rectangle does not prove that no window is shown.

## Observed states

| State | `0x60` | `0xF2` | `0xF4` | `0xF5` | `0xA5` | `0x95..0x98` |
| --- | --- | --- | --- | --- | --- | --- |
| Single, USB-C | `0x31` | `0x0100` | `0x0200` | `0x0402` (stale) | 0 | 0,0,0,0 |
| True Split 50/50 | `0x31` (left only) | `0x0103` | `0x0201` | `0x0402` (stale) | 0 | stale from an earlier PIP attempt |
| PIP attempt | `0x31` | `0x0101` | `0x0201` | `0x0402` (stale) | 0 | rectangle in pixels |

True Split reports only the left input in the low byte of `0x60`; the right
source is not exposed through DDC in that mode. Classification therefore uses
the low byte of `0xF2` first (`1` PIP, `3` True Split), and falls back to `0xF5`
and a valid rectangle only when `0xF2` is neither.

## Rectangle geometry

`0x95..0x98` are panel pixels. On the 5120x1440 model the reported maxima are
4960 / 1320 / 5120 / 1440 (the smaller left/top maxima are the OSD margins).
A quarter-screen window is therefore:

| Corner | left | top | right | bottom |
| --- | --- | --- | --- | --- |
| Top left | 0 | 0 | 2560 | 720 |
| Top right | 2560 | 0 | 5120 | 720 |
| Bottom left | 0 | 720 | 2560 | 1440 |
| Bottom right | 2560 | 720 | 5120 | 1440 |

Writing these values while the monitor is still in single mode succeeds and
reads back exactly, with `0xF4` unchanged. The rectangle can therefore be
staged before opening the PIP window, which avoids leaving the monitor in a
split with an unplaced window if a later write fails.

The first implementation wrote `0/50/100` here, treating the registers as
percentages. The monitor interpreted them as pixels and produced a 100-pixel
window near the top-left corner.

## Safe write sequences

### Leave a split and return to one picture

1. `0x60 = (source << 8) | source` (puts the same source in both halves).
2. `0xA5 = 0` (EIO is expected; the write can still apply).
3. `0x60 = source`.
4. Verify with `0xF4 = 0x0200` and `0x60` low byte = source.

This pair-first sequence is what made a stuck PIP/PBP state recoverable without
power-cycling the monitor.

### Enter True Split

1. `0xA5 = 0x0200`.
2. `0xF5 = 0x0101` or `0x0401`.
3. `0x60 = (right << 8) | left`.
4. Verify `0xF2` low byte = 3 and `0xF4 = 0x0201`.

Only 50/50 and about 70/30 changed the picture on the test unit. The other
layout codes were accepted but ignored or rejected with `DDCRC_VERIFY`.

### Enter PIP

1. While single, write the pixel rectangle to `0x95..0x98`.
2. Write `0xA5 = 0x0100` (EIO is expected).
3. Verify the rectangle still reads back and `0xF4 = 0x0201`.
4. Only then write `0x60 = source` for the PIP window.

Do not write the PIP source before step 3. If `0xA5` did not select the PIP
window, `0x60` changes the **main** input instead. The application then shows a
black or standby picture on the main area while the inactive source sits in the
quarter.

Do not write `0xA5 = 0x0100` while a PBP window is active. On the test unit this
froze the OSD and left one half in standby until the monitor was power-cycled.
Close PBP through the single-picture sequence first.

`0xA4` is not part of the working sequence. The monitor reads it as unsupported,
and a zero mask blanks the picture. The rectangle plus `0xA5` was sufficient.

## Failure modes seen

- **Black screen after PIP**: the source was written to `0x60` before the
  selector was verified. Since the selector was ignored, the source write
  changed the main input to an inactive input.
- **Half-screen standby / frozen OSD**: `0xA5 = 0x0100` while PBP was active.
  A full single-picture write and, in one case, an AC power cycle recovered it.
- **100-pixel PIP window**: rectangle written as percent instead of pixels.
- **True Split with no EDID on DP**: after switching to True Split, the DP
  connector reported a 0-byte EDID and only a 640x480 fallback mode; DDC
  disappeared as well, so software could not switch back. The desktop
  environment then enabled the internal panel. Disabling/enabling the DP output
  did not restore the EDID; only a monitor power cycle did. It is not known
  whether this is a monitor firmware limitation, a link-training issue or a
  cable issue. The app should not claim that True Split succeeded when the DP
  EDID disappears and should keep a recovery path that does not require DDC.
- **Stale split data**: `0xF4 = 0x0201`, `0xF5 = 0x0402` and an old PIP
  rectangle can all survive a mode change. Never use any single one of them as
  proof of the current picture.

## Refresh rate (reported, not verified)

A Windows 11 run placed USB-C in the PIP corner and HDMI 1 in the main picture
successfully, but only after the operating system switched the display to
60 Hz. At a higher refresh rate the PIP image was rendered at a lower resolution
and stretched. The earlier Linux failure (True Split, then a 0-byte DP EDID and
only 640x480) happened at the desktop's default high refresh.

Working hypothesis: the split/PIP scaler path cannot sustain the full
5120x1440@165 link, especially when DSC is in use. At 60 Hz the link has enough
margin; above it the monitor may degrade the picture or drop link training and
EDID.

Verification plan: set DP-2 to 5120x1440@60 in the OS display settings, confirm
a non-empty EDID and DDC access, then enable True Split and PIP. Repeat at
120/165 Hz and record when EDID, DDC or the picture fails. Do not treat the
60 Hz requirement as confirmed until this runs on Linux.

## Rescue baseline

A known-good state is the tuple `(0x60 source, 0xF2, 0xF5, brightness,
contrast)` captured while `0xF4` is `0x0200` and `0x60` is a single byte. The
restore sequence is the single-picture sequence above plus brightness and
contrast. Arming must not close a working split by itself; restore on a
disconnected-to-connected transition, or on an explicit user action.

If DDC is unavailable, the app cannot restore anything. The armed state and the
baseline live in the settings file so they survive a restart and can be applied
after the monitor returns.

## Validation

- Unit tests cover rectangle decoding, mode classification, the single-picture
  plan, the PIP plan order and the Rescue plan.
- Mock scenarios cover single 16:9/21:9/32:9, PBP, PIP, a stale True Split state
  and an offline monitor.
- Real hardware was used for the register sequences above. A supervised test is
  still required after any change to the True Split or PIP write order.
