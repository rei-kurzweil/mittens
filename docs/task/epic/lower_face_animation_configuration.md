# Epic: lower-face animation configuration

Date: 2026-09-21
Status: proposed

## Outcome

Creators can save and reuse deliberate lower-face response settings without
mixing together microphone measurement, loudness conditioning, avatar-specific
morph mapping, and future speech/phoneme analysis.

This epic owns the authoring boundary between a retained microphone level and a
lower-face animation response. It does not replace the separate
[microphone-driven visemes](visemes.md) epic.

## Current first slice

The present implementation intentionally covers only AVC's amplitude fallback:

```text
AudioInput -> Amplitude -> optional VolumeNormalization -> AVC -> viseme_aa
```

AVC owns the final RMS-to-mouth mapping. It supports exact
`mouth_open_rms_floor` / `mouth_open_rms_ceiling` calibration and the equivalent
centred `mouth_open_rms_center_range(center, range)` form, plus
`mouth_open_amount` for visual extent.

The first reusable profile is [Rei 2026.9](../../../assets/components/mouth_response/rei_2026.9.mms).
It returns only this stable data shape:

```mms
{
    rms_center = 0.038
    rms_range = 0.060
    mouth_movement_amount = 1.0
}
```

The focused implementation task is [AVC mouth-response preset factories](../avc-mouth-response-preset-factories.md).

## Boundaries

| Layer | Owns |
| --- | --- |
| `Amplitude` | raw PCM measurement |
| `VolumeNormalization` | optional adaptive loudness/AGC conditioning |
| `AvatarControl` | level-to-fallback-mouth mapping |
| `MorphTargetMap` | semantic channel to model morph target mapping |
| future phoneme/viseme analysis | speech-derived lower-face channels |

“Loudness stream” or “streaming loudness model” is provisional prose for a
retained level provider such as `Amplitude` or `VolumeNormalization`; it is not
a component name or public abstraction yet.

## Later phases

1. Decide whether a profile should remain a plain MMS table, gain a typed MMS
   struct, or become a host component/resource with versioned validation.
2. Define an explicit application helper that maps a profile onto AVC without
   silently replacing source, smoothing, or morph-map choices.
3. Decide how an AVC fallback profile coexists with phoneme/viseme-driven mouth
   animation and which driver has precedence.
4. Establish a stable term and component surface for streaming phoneme analysis;
   do not prematurely rename the existing `Visemes` epic or components.
5. Consider model-specific lower-face profiles only after the generic response
   values and application semantics are stable.

## Non-goals

- redesigning AGC policy or its retained diagnostics;
- inferring presets automatically from an audio device;
- committing to `Phonemes` versus `Visemes` naming; or
- expanding the three-value placeholder into a general facial-expression mixer.
