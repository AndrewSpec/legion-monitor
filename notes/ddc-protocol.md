# DDC/CI notes: Legion R45w-30

Technical notes from developing the Multi-view (PBP / PIP) and Rescue
features, including the monitor's hardware True Split protocol, which the app
does **not** expose because it blanked the panel at 120 Hz on 2026-11-12. They describe the monitor's DDC/CI behaviour as observed on the test
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
| `0x60` | Input source | Single byte = selected input. In PBP a pair `(right << 8) | left` sets both halves; True Split keeps a single byte because both displays show one input. In PIP the high byte is not stored and reads back as the low byte. |
| `0x95` | PIP rectangle left | Pixels, not percent |
| `0x96` | PIP rectangle top | Pixels |
| `0x97` | PIP rectangle right | Pixels |
| `0x98` | PIP rectangle bottom | Pixels |
| `0xA4` | PIP/PBP window mask | Reads as unsupported/all-zero. `0xFFFF` was rejected by `ddcutil`; the app does not write it. |
| `0xA5` | Window select | `0` close/full, `0x0100` PIP/background window, `0x0200` PBP window. Reads back as 0 even after it takes effect. |
| `0xD6` | Power mode | `1` on, `4` off; a DDC power cycle did not reset mode registers. |
| `0xF0` | Audio/source pair | Low byte left, high byte right on models that report it. |
| `0xF2` | PIP/PBP mode | **Current value is the mode:** `0x0000` single, `0x0100` PIP, `0x0101` PIP with the pictures swapped, `0x0103` a split. Bit 1 only says "split"; the app treats every split as PBP and does not probe `0xF7` during normal readback. Writes: `0` closes the window, `1` swaps main/PIP. Maximum is `0x0001`. |
| `0xF4` | Selected window mirror | Not a mode. Observed `0x0101` in PIP, `0x0100` after a swap and `0x0100` while single. Maximum is `0x0201`. Do not classify the mode from it. |
| `0xF5` | Split layout | `0x0101` = 50/50, `0x0401` = about 70/30. The app writes only `0x0101` (PBP); the wider value belongs to the hardware True Split. Reads 0 after leaving a split; `0x0402` is the maximum. |
| `0xF7` | Monitor feature value | Only meaningful after `0xF8` selects a feature. With `0xF8 = 0x15` it is the True Split switch: `0` off, `1` on. |
| `0xF8` | Monitor feature index | Selects which feature `0xF7` reads or writes. `0x15` = True Split. |

A zero PIP rectangle does not prove that no window is shown.

## Observed states

| State | `0x60` | `0xF2` current | `0xF4` current | `0xF5` current | `0xA5` | `0x95..0x98` |
| --- | --- | --- | --- | --- | --- | --- |
| Single, USB-C | `0x31` | `0x0000` | `0x0100` (stale) | 0 | 0 | 0,0,0,0 or stale |
| PBP 50/50 (2026-11-12) | `0x31` (left only) | `0x0000` | `0x0000` | `0x0101` | 0 | stale from an earlier PIP attempt |
| PBP 50/50 (earlier observation) | `0x31` (left only) | `0x0103` | `0x0101` | 0 | 0 | stale from an earlier PIP attempt |
| PIP, main HDMI 1 / corner USB-C | `0x31` | `0x0100` | `0x0100` | 0 | 0 | 2560,0,5120,720 |
| PIP, swapped | `0x31` | `0x0101` | `0x0101` | 0 | 0 | same rectangle |

PBP can report only the left input in the low byte of `0x60`; the right source
is not always exposed through DDC. In the 2026-11-12 hardware run `0x60` stayed
at `0x0031` in PBP, while `0xF0` reported the full pair: `0x0F31` right after
opening and `0x1131` after changing the right half to HDMI 1. A source change
therefore writes the full pair to `0x60` (and the app also reads `0xF0` as the
pair fallback). The pair write itself can answer `EIO` while still applying.

Classification cannot rely on `0xF2` alone: on this unit PBP left F2 at
`0x0000` while the split was physically open. The app treats the state as PBP
when F2 says single **and** F5 holds a valid layout **and** the compositor
reports a half-width output (2560 instead of the native 5120). The half-width
EDID is the decisive physical signal; F5 alone can be stale. Do not select
`0xF8 = 0x15` and read `0xF7` during normal polling: that probe disturbed the
following F2/F5/0x60 reads and made PBP look like True Split. `0xF4` mirrors the
selected window and is not a mode register.

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

### Leave True Split

1. `0xF8 = 0x15` (select the feature).
2. `0xF7 = 0` (switch it off).
3. Wait until `0xF2 = 0`; the DP link renegotiates and the register can lag.
4. Verify `0xF7` reads `0` and `0xF2` reports one picture.

Do **not** use the PBP/PIP exit here: writing `0xA5 = 0x0100` while True Split
is active froze the OSD in testing.

### Enter PBP

1. `0xF8 = 0x15`, `0xF7 = 0` (clear a legacy hardware True Split; harmless when
   it is already off). The app does this because an OSD-enabled True Split
   would feed both PBP halves from one source.
2. `0xA5 = 0x0200`.
3. `0xF5 = 0x0101` (this monitor applies only 50/50 in PBP).
4. `0x60 = (right << 8) | left`; seeding both bytes with the current input makes
   the pair readable so either half can be changed afterwards.
5. Verify the split physically: F5 reads `0x0101` and the desktop output is
   2560x1440 (half of the native 5120x1440). F2 can stay `0x0000` on this unit;
   accept that when F5 and the half-width EDID agree. Do **not** read `0xF7`
   here: the extra F8/F7 probe disturbed the following F2/F5/0x60 reads and
   broke PBP state display.
6. Changing a half writes one full `0x60` pair. `0x60` may keep reporting only
   the left byte; read `0xF0` to confirm the applied pair.

The other F5 layout codes belong to the hardware True Split and were accepted
but ignored or rejected with `DDCRC_VERIFY` in PBP.

### Enter True Split (not exposed by the app)

True Split is a USB-C feature on this unit. Require `0x60 & 0xFF` in
`0x31..=0x3F` first.

1. If PIP or PBP is active, close it with the matching sequence and wait for
   `0xF2 = 0`.
2. `0xF8 = 0x15` (select the True Split feature).
3. `0xF7 = 1` (switch it on).
4. `0xF5 = 0x0101` or `0x0401` (50/50 or about 70/30).
5. Verify `0xF7` reads `1`; the system should now see two displays.

The `0xF8`/`0xF7` sequence comes from Lenovo's background service
(`SetVCPFeature` at `0x4ac219`, `GetVCPFeature` at `0x4ac253`, `SetVCPFeature`
at `0x4ac2cd` in the service disassembly). It was run against the physical
monitor on 2026-11-12: over USB-C the link became 2560x1440 at 120 Hz, the panel
went black and DDC disappeared. The compositor owns the refresh rate, so the app
cannot undo that. Only the off step (`0xF8 = 0x15`, `0xF7 = 0`) remains in the
app, as part of Rescue and PBP cleanup.

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
- **Split entry with no EDID on DP**: the old UI labelled the two-source PBP
  write "True Split". After entering that split, the DP connector reported a
  0-byte EDID and only a 640x480 fallback mode; DDC disappeared as well, so
  software could not switch back. The desktop environment then enabled the
  internal panel. Disabling/enabling the DP output did not restore the EDID;
  only a monitor power cycle did. It is not known whether this is a monitor
  firmware limitation, a link-training issue or a cable issue. The app should
  not claim that a split succeeded when the DP EDID disappears and should keep
  a recovery path that does not require DDC. The hardware True Split path also
  renegotiates the link (one 5120x1440 output becomes two displays), so the
  same caution applies; it has not been tested on this monitor yet.
- **Stale split data**: an old PIP rectangle can survive a mode change, and the
  `0xF5`/`0xF4` maxima are easy to mistake for current values. `0xF2` current
  value is the reliable mode signal; verify it rather than a cached UI hint.

## Refresh rate (reported, not verified)

A Windows 11 run placed USB-C in the PIP corner and HDMI 1 in the main picture
successfully, but only after the operating system switched the display to
60 Hz. At a higher refresh rate the PIP image was rendered at a lower resolution
and stretched. The earlier Linux failure (the split write then labelled True
Split, followed by a 0-byte DP EDID and only 640x480) happened at the desktop's
default high refresh.

Working hypothesis: the split/PIP scaler path cannot sustain the full
5120x1440@165 link, especially when DSC is in use. At 60 Hz the link has enough
margin; above it the monitor may degrade the picture or drop link training and
EDID.

Verification plan: set DP-2 to 5120x1440@60 in the OS display settings, confirm
a non-empty EDID and DDC access, then enable PBP and PIP. Repeat at
120/165 Hz and record when EDID, DDC or the picture fails. Do not treat the
60 Hz requirement as confirmed until this runs on Linux.

## Rescue baseline

A known-good state is the tuple `(0x60 source, 0xF2, 0xF5, brightness,
contrast)` captured while `0xF2 = 0` and `0x60` is a single byte. A baseline is
optional: "Restore now" always works while the monitor answers DDC and falls
back to the current input when no baseline exists. Restore always writes
`0xF8 = 0x15`, `0xF7 = 0` (clear a legacy hardware True Split), `0xF5 = 0`,
`0xF2 = 0`, `0xA5 = 0`, then the single source; brightness and contrast follow
when a baseline stored them.
Arming must not close a working split by itself; restore on a
disconnected-to-connected transition, or on an explicit user action.

If DDC is unavailable, the app cannot restore anything. The armed state and the
baseline live in the settings file so they survive a restart and can be applied
after the monitor returns.

## Validation

- Unit tests cover rectangle decoding, mode classification, the single-picture
  plan, the PIP plan order and the Rescue plan.
- Mock scenarios cover single 16:9/21:9/32:9, PBP, PIP, a stale split state and
  an offline monitor. The `true-split` scenario is kept only as a protocol
  preview; the UI no longer uses it.
- Real hardware was used for the register sequences above. A supervised test is
  still required after any change to the PBP or PIP write order.
