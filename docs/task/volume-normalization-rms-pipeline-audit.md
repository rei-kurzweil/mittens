# Volume-normalization RMS pipeline audit

Date: 2026-09-21

## Question

Does this scene apply RMS twice?

```mms
let microphone = AudioInput {}
let raw_voice_level = Amplitude.rolling_window(0.080).from(microphone) {}
let voice_level = VolumeNormalization.from(raw_voice_level) {}
```

## Current result

No. The two retained levels are not serial RMS operations.

`Amplitude` and `VolumeNormalization` each receive the same microphone PCM
frames from the audio callback and each owns a separate 80 ms rolling RMS/peak
window. The former retains raw RMS/peak. The latter computes its own raw window
from those frames, applies its current virtual analysis gain to that result,
and retains the adjusted RMS/peak. It does not feed the already-computed
`Amplitude` RMS into another RMS calculation, and it never changes audible PCM.

```text
AudioInput PCM frames
  ├─ Amplitude rolling RMS window ──────────────> raw_voice_level
  └─ VolumeNormalization rolling RMS + AGC gain ─> voice_level
                                                    └─ AVC mouth mapping
```

The duplicate rolling window is intentional: AGC must retain its own controller
state and adjusted snapshot while the raw meter stays truthful and available to
other consumers. It may be worth sharing raw-window work in a future
performance pass, but sharing must not make raw diagnostics depend on AGC
lifecycle or policy.

## Why the gain may remain at 0 dB

The normalizer begins at unity (0 dB). With the current policy it remains there
when activity is in the target band, below the activity gate, or above the high
threshold for less than the 0.75-second hold. It raises only after sustained
above-gate input below the target band, and it attenuates only after sustained
input above the high target (or immediately for peak headroom protection).

Therefore a responsive raw/normalized level display plus a flat 0 dB gain is a
valid controller state, but it needs a deliberate live test before treating it
as correct for a particular microphone.

## Ownership decision

RMS floor/ceiling belong to `AvatarControl`, not `Amplitude`:

- `Amplitude` describes measured PCM and should not acquire avatar-specific
  mouth semantics.
- `VolumeNormalization` owns source conditioning and AGC policy/gain.
- `AvatarControl` owns the mapping of a chosen retained level provider into a
  particular mouth morph.

AVC supports both exact `mouth_open_rms_floor` / `mouth_open_rms_ceiling` and
the equivalent centred `mouth_open_rms_center_range(center, range)` form.

## Next verification

1. Add an explicit retained controller reason read (`Holding`, `Raising`,
   `SustainedReducing`, etc.) to the debugging UI only when a concrete panel
   needs it.
2. Run repeatable quiet, sustained-normal, sustained-loud, and peak tests with
   the desktop microphone scene; capture raw RMS, normalized RMS, gain dB, and
   reason at 100 ms intervals.
3. Decide from measurements whether the default activity gate, target band, or
   hold durations should change. Do not tune AVC mouth bounds to conceal an AGC
   policy issue.
