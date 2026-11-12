# DDC/CI notes: Legion R45w-30

Technical notes from developing the Multi-view (PBP / PIP) and Rescue
features, including the monitor's hardware True Split protocol. The app exposes
True Split as a USB-C-only mode option: it activates the split first, waits for the
two screens, enables both external outputs (turning the built-in `eDP` panel
off if it appeared), and then lowers each external output to 60 Hz. Lowering before
activation broke the link at 120 Hz on 2026-11-12. They describe the monitor's DDC/CI behaviour as observed on the test
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
| `0x60` | Input source | Single byte = selected input. In PBP and PIP a pair is written. In PBP the pair is `(right << 8) | left`. In PIP the low byte is the main picture and the high byte the PIP source. The `0x60` pair write is what creates a PIP/PBP window; it makes the monitor re-enumerate for several seconds. The firmware may keep the previously valid pair (e.g. `0x1131`) when asked for a same-source pair (`0x3131`), while still opening the window. |
| `0x95` | PIP rectangle left | Pixels, not percent |
| `0x96` | PIP rectangle top | Pixels |
| `0x97` | PIP rectangle right | Pixels |
| `0x98` | PIP rectangle bottom | Pixels |
| `0xA4` | PIP/PBP window mask | Raw writes are accepted on the R45w-30 (the earlier `ddcutil` rejection was a write-encoding artifact). The app preflights `0xFFFF`: if accepted it uses the `0xA4 = 0` → rect → `0xA4 = 0xFFFF` order with retries; if the preflight is rejected it skips the mask, so a failed hide cannot blank the panel. |
| `0xA5` | Window select | `0` close/full, `0x0100` PIP window, `0x0200` PBP window. Writable but **write-only**: it always reads back 0, and on its own it does not change `0xF4` or open a window. The `0x60` pair write is the opener. |
| `0xD6` | Power mode | `1` on, `4` off; a DDC power cycle did not reset `0xF4`/`0xF5`. |
| `0xF0` | Audio/source pair | Low byte left, high byte right on models that report it. |
| `0xF2` | PIP/PBP swap | **Not a mode register.** It stays `1` (maximum `1`) in single, PIP and PBP. Writes: `0` closes the window, `1` swaps main/PIP, `3` leaves full-screen. Do not use it to detect or open a window. |
| `0xF4` | **Window state** | The authoritative mode. `0x0100`/`0x0101` = PIP (swapped bit 0), `0x0200`/`0x0201` = PBP, anything else single. True Split reports the same PBP value and is distinguished by two external outputs plus a set `F7`. Read-only in practice: writes trigger a scaler re-enumeration but do not persist. Maximum `0x0201`. |
| `0xF5` | Split layout | `0x0101` = 50/50, `0x0401` = about 70/30. The app writes only `0x0101` (PBP); the wider value belongs to the hardware True Split. Reads 0 after leaving a split; `0x0402` is the maximum (True Split 1:1:1). |
| `0xF7` | Monitor feature value | Only meaningful after `0xF8` selects a feature. With `0xF8 = 0x15` it is the True Split mode: `0` off, `1` 1:1, `3` 3:1, `2` 1:3, `4` 1:1:1. |
| `0xF8` | Monitor feature index | Selects which feature `0xF7` reads or writes. `0x15` = True Split. |
| `0xFA` | Feature mutex | Write `feature << 8`, wait 50 ms, read; `token\|1` or `1` means locked. |

A zero PIP rectangle does not prove that no window is shown.

## Observed states

| State | `0x60` | `0xF4` current | `0xF5` current | `0xA5` | `0x95..0x98` |
| --- | --- | --- | --- | --- | --- |
| Single, USB-C | `0x31` | `0x0000` | 0 | 0 | stale possible |
| PBP 50/50 | pair (e.g. `0x1131`) | `0x0200` | `0x0101` | 0 | stale possible |
| PIP, main USB-C / PIP HDMI 1 | `0x1131` | `0x0100` | 0 | 0 | 0,0,1280,720 |
| PIP, swapped | `0x1131` | `0x0101` | 0 | 0 | same rectangle |

PBP can report only the left input in the low byte of `0x60`; the right source
is not always exposed through DDC. In the 2026-11-12 hardware run `0x60` stayed
at `0x0031` in PBP, while `0xF0` reported the full pair: `0x0F31` right after
opening and `0x1131` after changing the right half to HDMI 1. A source change
therefore writes the full pair to `0x60` (and the app also reads `0xF0` as the
pair fallback). The pair write itself can answer `EIO` while still applying.

Classification uses `0xF4`: `0x0100`/`0x0101` is PIP, `0x0200`/`0x0201` is PBP,
anything else single. Only when `0xF4` is unavailable does the app fall back to
`0xF5` plus the half-width compositor output (2560 instead of 5120). `0xF2` and
`0xF5` alone are not sufficient. Do not select `0xF8 = 0x15` and read `0xF7`
during normal polling: that probe disturbed the following reads.


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
reads back exactly. The rectangle must be written **before** the PIP window is
created: the `0x60` pair write that opens the window does not reset it, while the
old sequence that opened first and placed afterwards left the default top-right
quadrant. The app writes mask → rectangle → show, then `0xA5 = 0x0100`, then the
`0x60` pair, so a failure after opening cannot leave an unplaced window.

The first implementation wrote `0/50/100` here, treating the registers as
percentages. The monitor interpreted them as pixels and produced a 100-pixel
window near the top-left corner.

## Safe write sequences

### Leave a split and return to one picture

The app's single plan is: `0xF8 = 0x15`, `0xF7 = 0`, `0xF5 = 0`, `0xF2 = 0`,
`0xA5 = 0`, then `0x60 = source`. The monitor may answer `EIO` and disappear for
a few seconds while it re-enumerates; the plan tolerates that and verifies the
result afterwards:

1. `0xF8 = 0x15`, `0xF7 = 0` (clear a legacy True Split).
2. `0xF5 = 0` (clear a stale layout).
3. `0xF2 = 0` (close the PIP/PBP window).
4. `0xA5 = 0` (return to the full-screen window).
5. `0x60 = source` (the input the app is running on).
6. Verify `0xF4` reports a single picture (`0x0000`/other non-split value) and
   the low byte of `0x60` equals the source.

Do not close first and write the source afterwards if the window selector is
unknown; that can leave the other input full-screen. The explicit `0xA5 = 0`
before the source write keeps the main window targeted.

### Leave True Split

1. `0xF8 = 0x15` (select the feature).
2. `0xF7 = 0` (switch it off).
3. Wait for the link to renegotiate; `0xF4`/`0xF5` can lag.
4. Verify `0xF4` reports one picture and `0xF5` is `0`.

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
5. Verify `0xF4` reads `0x0200`/`0x0201` (PBP) and `0xF5` reads `0x0101`; the
   desktop output should be 2560x1440 (half of the native 5120x1440). Do **not**
   read `0xF7` here: the extra F8/F7 probe disturbed the following reads.
6. Changing a half writes one full `0x60` pair. `0x60` may keep reporting only
   the left byte; read `0xF0` to confirm the applied pair.

The other F5 layout codes belong to the hardware True Split and were accepted
but ignored or rejected with `DDCRC_VERIFY` in PBP.

### Enter True Split (Multi-view True Split mode)

True Split is a USB-C feature on this unit. Require `0x60 & 0xFF` in
`0x31..=0x3F` first.

1. If PIP or PBP is active, close it with the matching sequence and wait for
   `0xF4` to report a single picture.
2. `0xF8 = 0x15` (select the True Split feature).
3. `0xF7 = 1/3/2/4` (1:1, 3:1, 1:3 or 1:1:1).
4. Wait about 6 s for the monitor to re-enumerate into two 2560×1440 screens.
5. Enable every **external** output; if the desktop enabled the internal `eDP`
   panel, disable it. Set each external output to its highest mode at or below
   60 Hz, at the resolution the split gave it (`kscreen-doctor
   output.<name>.enable` / `.mode.<id>`; XRandR fallback).
6. Verify `0xF7` after DDC returns; the desktop should now have two external
   screens. Do not write `0xF5` — the monitor owns the split layout.

On the physical monitor this sequence produced 2560x1440 at 120 Hz over USB-C;
the panel went black and DDC disappeared. The fix is to activate the split
first and lower the two resulting screens afterwards; lowering the pre-split
desktop is wrong because that mode does not exist after re-enumeration.
The off step (`0xF8 = 0x15`, `0xF7 = 0`) remains part of Rescue and PBP cleanup.

### Enter PIP

1. Require `0xF4` to report single (no PIP/PBP) before starting. If a split is
   active, close it first and wait for the link.
2. Preflight `0xA4 = 0xFFFF`; if accepted write `0xA4 = 0`, the rectangle
   `0x95..0x98` in panel pixels, and `0xA4 = 0xFFFF` with retries. If the
   preflight is rejected, skip the mask and write the rectangle anyway.
3. Write `0xA5 = 0x0100`. This is write-only and does not open the window on its
   own; do not judge it by readback.
4. Write `0x60 = (pip_source << 8) | main_source`. **This write creates the PIP
   window.** The monitor typically re-enumerates for 5–7 s and DDC may answer
   `EIO`/`-3020`; wait, reconnect, and read `0xF4`.
5. Require `0xF4 = 0x0100` (or `0x0101` after a swap) and the rectangle readback
   to match. If `0xF4` is not PIP, rewrite the `0x60` pair once and wait again.
   Do not require the `0x60` readback to match: the firmware may keep a
   previously valid pair (`0x1131`) when it rejects a same-source pair
   (`0x3131`) while still opening the window.

`0xF2` is **not** used to open PIP. It is only the swap action (`0xF2 = 1`) and
the close action (`0xF2 = 0`). The earlier sequence `A5=0x0100` → `F2=1` →
rectangle was wrong: it opened nothing and placed the rectangle after the
window was supposed to exist.

Do not write `0xA5 = 0x0100` while a PBP window is active. On the test unit this
froze the OSD and left one half in standby until the monitor was power-cycled.
Close PBP through the single-picture sequence first.

`0xA4` is written with a show-value preflight; a rejected `0xFFFF` skips the
mask entirely, so a failed hide cannot blank the panel.

## Failure modes seen

- **Black PIP inset**: `0xF4` reports PIP but the quarter has no picture. Either
  the PIP source has no signal, or the firmware kept an older pair (`0x1131`)
  instead of the requested same-source pair (`0x3131`). Select a connected
  second input; do not treat a matching pair readback as proof.
- **Main picture changed instead of PIP**: a single-byte `0x60` write after
  `0xA5 = 0x0100` changes the main input when no window was created. Always
  write the full pair after the selector.
- **Single restore leaves the other input**: the old sequence closed the window
  before writing `0x60`; the write no longer reached the background. Select the
  background (`0xA5 = 0`), write the source, then close.
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
  a recovery path that does not require DDC.
- **Stale split data**: an old PIP rectangle can survive a mode change, and the
  `0xF4`/`0xF5` maxima are easy to mistake for current values. `0xF4` current
  value is the reliable mode signal; verify it rather than a cached UI hint.

## Refresh rate (required, switched automatically)

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

Verified on 2026-11-12: with DP-2 at 5120x1440@120 the live PIP test called
`kscreen-doctor output.DP-2.mode.2` automatically, DDC returned, and PIP opened
with `0xF4 = 0x0100` and the expected rectangle. All four corners (TR, TL, BL,
BR) were then verified individually through `live_pip_corner_roundtrip`; each
returned the expected rectangle and `F4 = 0x0100` or `0x0101`. The worker does
this for PIP and PBP before the split write. True Split is the exception: it is
activated first, and the post-activation topology is lowered per external
output. The UI only explains which of the two orders applies.

## Rescue baseline

A known-good state is the tuple `(0x60 source, 0xF4, 0xF5, brightness,
contrast)` captured while `0xF4` reports neither PIP nor PBP and `0x60` is a
single byte. A baseline is
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
- Mock scenarios cover single 16:9/21:9/32:9, PBP, PIP, True Split, a stale
  split state and an offline monitor. The `true-split` scenario is what the
  English documentation screenshots use for the two-screen state.
- Real hardware was used for the register sequences above. A supervised test is
  still required after any change to the PBP or PIP write order.
