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
  Treat the error as inconclusive and verify the resulting state.
- `0x60` reads back `Invalid value` for known inputs (for example `0x31` for
  USB-C). That is a display quirk, not a write failure.

## ddcutil output format

`CNC` prints `mh ml sh sl`: `mh`/`ml` are the feature's **maximum**, `sh`/`sl`
are the **current** value. `SNC` prints only the current byte. This matters:
`0xF2 CNC 00 01 00 00` means current 0 with maximum 1, not current 1. The parser
in `src/ddc.rs` already combines `sh`/`sl` for the current value.

## Registers used

| Code | Meaning | Notes |
| --- | --- | --- |
| `0x10` | Brightness | 0..100 |
| `0x12` | Contrast | 0..100 |
| `0x60` | Input source | Single byte = selected/window input. A pair `(right << 8) | left` works for True Split; in PIP the high byte is not stored and reads back as the low byte. |
| `0x95` | PIP rectangle left | Pixels, not percent |
| `0x96` | PIP rectangle top | Pixels |
| `0x97` | PIP rectangle right | Pixels |
| `0x98` | PIP rectangle bottom | Pixels |
| `0xA4` | PIP/PBP window mask | Reads as unsupported/all-zero. `0xFFFF` was rejected by `ddcutil`; the app does not write it. |
| `0xA5` | Window select | `0` close/full, `0x0100` PIP/background window, `0x0200` PBP window. Reads back as 0 even after it takes effect. |
| `0xD6` | Power mode | `1` on, `4` off; a DDC power cycle did not reset mode registers. |
| `0xF0` | Audio/source pair | Low byte left, high byte right on models that report it. |
| `0xF2` | PIP/PBP mode | **Current value is the mode:** `0x0000` single, `0x0100` PIP, `0x0101` PIP with the pictures swapped, `0x0103` True Split. Writes: `0` closes the window, `1` swaps main/PIP. Maximum is `0x0001`. |
| `0xF4` | Selected window mirror | Not a mode. Observed `0x0101` in PIP, `0x0100` after a swap and `0x0100` while single. Maximum is `0x0201`. Do not classify the mode from it. |
| `0xF5` | True Split layout | `0x0101` = 50/50, `0x0401` = about 70/30. Reads 0 after leaving a split; `0x0402` is the maximum. |

A zero PIP rectangle does not prove that no window is shown.

## Observed states

| State | `0x60` | `0xF2` current | `0xF4` current | `0xF5` current | `0xA5` | `0x95..0x98` |
| --- | --- | --- | --- | --- | --- | --- |
| Single, USB-C | `0x31` | `0x0000` | `0x0100` (stale) | 0 | 0 | 0,0,0,0 or stale |
| True Split 50/50 | `0x31` (left only) | `0x0103` | `0x0101` | 0 | 0 | stale from an earlier PIP attempt |
| PIP, main HDMI 1 / corner USB-C | `0x31` | `0x0100` | `0x0100` | 0 | 0 | 2560,0,5120,720 |
| PIP, swapped | `0x31` | `0x0101` | `0x0101` | 0 | 0 | same rectangle |

True Split reports only the left input in the low byte of `0x60`; the right
source is not exposed through DDC in that mode. Classification uses `0xF2`:
zero is single, bit 1 is True Split, otherwise it is PIP. `0xF4` mirrors the
selected window and stays non-zero after the split closes.

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
reads back exactly. The rectangle must be written **after** opening PIP with
`F2 = 1`: opening resets it to the default top-right quadrant, so staging it
before opening only ever produced the top-right corner. It can be read before
opening, but
PIP window, which avoids leaving the monitor in a split with an unplaced window
if a later write fails.

The first implementation wrote `0/50/100` here, treating the registers as
percentages. The monitor interpreted them as pixels and produced a 100-pixel
window near the top-left corner.

## Safe write sequences

### Leave a split and return to one picture

1. `0xA5 = 0x0100` (select the background window; EIO is expected).
2. `0x60 = source` (the input the app is running on).
3. `0xF2 = 0` (close PIP/PBP).
4. `0xA5 = 0`.
5. Verify `0xF2 = 0` and `0x60` low byte = source.

Do not close first and write the source afterwards. That changed the mode but
left the other input full-screen because the write no longer targeted the
background window.

### Enter True Split

1. `0xA5 = 0x0200`.
2. `0xF5 = 0x0101` or `0x0401`.
3. `0x60 = (right << 8) | left`.
4. Verify `0xF2` bit 1 is set.

Only 50/50 and about 70/30 changed the picture on the test unit. The other
layout codes were accepted but ignored or rejected with `DDCRC_VERIFY`.

### Enter PIP

1. While single, write the pixel rectangle to `0x95..0x98`.
2. Write `0xA5 = 0x0100` (EIO is expected). This opens PIP and selects the
   background window.
3. Verify the rectangle still reads back and `0xF2` is PIP (`0x0100`, bit 1
   clear).
4. To place a different source in the quarter: write `0x60 = secondary` (this
   temporarily changes the background), then `0xF2 = 1` (swap). The app's own
   input returns to the background and the secondary source appears in the
   quarter.
5. If the secondary source equals the app input, skip step 4.

Do not write `0x60` while the selected window is unknown or before `0xA5`.
On the test unit the app wrote the source after `A5=0x0100`; the monitor applied
it to the **main** picture, so HDMI 1 became full-screen and USB-C stayed in the
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
- **Swapped main/PIP**: the PIP source was written without a following
  `0xF2=1`; the monitor applied it to the background. Write the source, then
  swap once.
- **Single restore leaves the other input**: the old sequence closed the window
  before writing `0x60`; the write no longer reached the background. Select the
  background, write the source, then close.
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
- **Stale split data**: an old PIP rectangle can survive a mode change, and the
  `0xF5`/`0xF4` maxima are easy to mistake for current values. `0xF2` current
  value is the reliable mode signal; verify it rather than a cached UI hint.

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
contrast)` captured while `0xF2 = 0` and `0x60` is a single byte. The restore
sequence is the single-picture sequence above plus brightness and contrast.
Arming must not close a working split by itself; restore on a
disconnected-to-connected transition, or on an explicit user action.

If DDC is unavailable, the app cannot restore anything. The armed state and the
baseline live in the settings file so they survive a restart and can be applied
after the monitor returns.

## Validation

- Unit tests cover rectangle decoding, mode classification, the single-picture
  plan, the PIP plan order and the Rescue plan.
- Mock scenarios cover single 16:9/21:9/32:9, True Split, PIP, a stale True
  Split state and an offline monitor.
- Real hardware was used for the register sequences above. A supervised test is
  still required after any change to the True Split or PIP write order.
