# Performer-facing mouth-response controls

Date: 2026-09-21

## Purpose

Give `AvatarControl` a small, understandable microphone-to-mouth baseline
without conflating that mapping with `VolumeNormalization` (AGC) policy.

`VolumeNormalization` supplies an AGC-adjusted retained RMS and reports the
actual signed virtual analysis gain through `gain_db()`. AVC does **not** add a
second fixed gain. It maps that RMS to the `viseme_aa` weight. The legacy
`mouth_open_rms_floor` and `mouth_open_rms_ceiling` fields are linear RMS
mapping points, not dB gain limits.

## Current baseline

`AvatarControl` now also exposes this equivalent, performer-facing vocabulary:

```mms
AVC {
    mouth_open_rms_center_range(0.0315, 0.057)
    mouth_open_amount(1.0)
}
```

- `mouth_open_rms_floor(floor)` and `mouth_open_rms_ceiling(ceiling)` remain
  the exact, independent calibration builders.
- `mouth_open_rms_center_range(center, range)` is a second, equivalent form:
  it maps to `floor = center - range / 2` and `ceiling = center + range / 2`.
  The range must remain within non-negative RMS.
- `mouth_open_amount` caps the visual morph contribution in `0..=1`, without
  changing the trigger range.
- Runtime handlers can update the centred form through
  `set_mouth_open_rms_center_range(...)` and `set_mouth_open_amount(...)`; this
  changes the retained AVC component only.
  It does not reopen audio input, rebuild AGC, or recreate the avatar.

`assets/components/ui/mouth_response_panel.mms` now supplies the same three
controls to the desktop and XR AGC examples and to `examples/rei(mu).mms`.
The scene supplies its own AVC handle, response values, title, and description.
The panel remains compact for now:

1. **RMS centre** — midpoint of the level range mapped into mouth movement.
2. **RMS range** — full sensitivity span around that midpoint.
3. **Mouth amount** — maximum visual opening, independent of sensitivity.

The previous detailed AGC-policy sliders are intentionally absent from this
panel while this interaction is validated. The live readout and AGC gain
visualization remain the source of truth for diagnosing AGC behaviour.

## Follow-up work

1. Make the AGC history visualization demonstrably show non-unity gain during
   controlled quiet and sustained-loud microphone tests. Confirm whether a
   zero-gain graph represents correct policy state or a missing retained update.
2. Add a compact response preview: input RMS, calibrated closed/open points,
   and final mouth weight. It should use retained main-thread values only.
3. Decide whether the response should support an authored curve (for example,
   gamma or a soft knee) after the linear controls have been evaluated.
4. Reintroduce advanced AGC settings in a separate expandable section only
   after each has a clear live diagnostic and bounded layout.
5. Consider making this mapping reusable for other viseme/morph targets while
   preserving AVC's precedence rules for a future full viseme driver.
6. Add high-pass analysis controls only with the callback-side observer in
   [the high-pass amplitude task](high-pass-amplitude-observer.md).

## Acceptance criteria

- Adjusting any baseline control visibly updates Bisket without reloading the
  scene or audio device.
- A small RMS range is less sensitive; a larger range is more sensitive, with
  the midpoint held fixed.
- Mouth amount changes visual extent only.
- Slider tracks and thumbs remain inside the information panel at all values.
- AGC gain diagnostics remain independent: no mouth calibration control may
  masquerade as applied AGC gain.
