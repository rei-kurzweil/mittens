# Task: AVC mouth-response preset factories

Date: 2026-09-21
Parent: [Lower-face animation configuration](epic/lower_face_animation_configuration.md)
Status: first placeholder implemented; application-helper design pending

## Goal

Allow a calibrated AVC mouth fallback response to be saved as a small,
importable MMS factory without binding it to a microphone, AGC policy, model, or
scene topology.

## First placeholder

`assets/components/mouth_response/rei_2026.9.mms` exports:

```mms
rei_2026_9() -> {
    rms_center = 0.038
    rms_range = 0.060
    mouth_movement_amount = 1.0
}
```

The desktop AGC example consumes the table as follows:

```mms
let preset = rei_2026_9()
AVC {
    mouth_open_rms_center_range(preset.rms_center, preset.rms_range)
    mouth_open_amount(preset.mouth_movement_amount)
}
```

These map directly to current AVC builder calls. Direct
`mouth_open_rms_floor` and `mouth_open_rms_ceiling` builders remain valid and
are not replaced by the preset form.

## Why plain data first

The table establishes parameter names and a reusable asset path before deciding
whether presets should mutate an existing component, return a component
fragment, support overrides, include smoothing, or carry profile metadata. A
factory must not silently choose the audio source, `VolumeNormalization` policy,
or `MorphTargetMap` target.

## Follow-up

1. Define a deliberately named application helper only after its ownership and
   override behavior are specified.
2. Decide whether presets include mouth smoothing and, if so, distinguish it
   from microphone/AGC smoothing.
3. Add profile versioning or typed MMS validation if more settings become
   durable public data.
4. Specify fallback precedence against the future phoneme/viseme driver.

## Acceptance criteria

- Importing the preset returns the documented three values.
- A scene can apply them through existing AVC builder calls.
- Existing direct floor/ceiling configuration remains supported.
- The preset cannot implicitly alter microphone capture, AGC, or a model's
  morph mapping.
