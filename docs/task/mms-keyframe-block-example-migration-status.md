# MMS Keyframe Block Example Migration Status

## Goal

Migrate authored MMS examples from legacy `Action.*` children under
`Keyframe.at(...)` to the new timed block model:

- `Keyframe.at(...) { ... }` captures live MMS component handles
- method calls inside the block dispatch intents at the due frame
- `ActionComponent` remains compatibility plumbing, not the preferred authoring API

## Status

### Migrated MMS examples

- [x] `examples/component-method-call.mms`
- [x] `examples/pride.mms`
- [x] `examples/transition.mms`
- [x] `examples/observer-router.mms`
- [x] `examples/bisket-vr-demo.mms`
- [x] `examples/router.mms`

### Migrated Rust example launchers

The `.mms` files alone were not enough. Any launcher that still used
`MeowMeowRunner::eval(...)` stayed in the old offline mode, which cannot
materialize live `let`-bound component handles for keyframe blocks.

- [x] `examples/component-method-call.rs`
- [x] `examples/pride.rs`
- [x] `examples/observer-router.rs`
- [x] `examples/bisket-vr-demo.rs`
- [x] `examples/router.rs`
- [x] `examples/transition.rs`

### What this migration needed

- [x] `KeyframeComponent` stores an executable captured MMS block
- [x] `AnimationSystem` invokes the block when the keyframe becomes due
- [x] live method calls from keyframe blocks push intents through the normal signal path
- [x] `Transform.update_transform(translation, rotation_euler, scale)` method for MMS
- [x] example launchers use live runner paths (`eval_with_world...`) when authored MMS depends on captured live handles

### What did not require new support

- No new host calls were needed for the remaining MMS example migration pass
- Existing live component capture semantics were sufficient once keyframe blocks were executable
- Existing `Emissive.set_intensity(...)` support covered the glow-animation examples

## Remaining gaps after MMS example migration

The authored MMS examples are migrated, but some Rust examples still construct
legacy `ActionComponent`s directly. Those examples point to missing MMS
surface area if we want to migrate them too.

### Missing MMS methods / intent shims

- `Transform.set_color(...)` or equivalent color-target method for live handles
  - Needed by Rust examples that keyframe `IntentValue::SetColor`
- `Text.set_text(...)` is already present
- `Transform.update_transform(...)` is now present

### Missing topology mutation methods

- `component.detach()`
  - maps to `IntentValue::Detach`
- `parent.attach(child)`
  - maps to `IntentValue::Attach`
- `parent.attach_clone(prefab)`
  - maps to `IntentValue::AttachClone`
- `parent.remove_child(index)`
  - maps to `IntentValue::RemoveChild`

These are needed to migrate topology-animation examples like:

- `examples/animation-for-topology.rs`
- parts of `examples/raycast-topology-animation.rs`

### Missing raycast-trigger method

- `raycaster.request_raycast()`
  - maps to `IntentValue::RequestRaycast`

Needed for:

- `examples/raycast-topology-animation.rs`

### Missing audio scheduling / parameter methods

- `audio_source.schedule_play(note, beat_offset, ...)`
  - maps to `IntentValue::AudioSchedulePlay`
- `band_pass.set_center_hz(...)`
  - maps to `IntentValue::AudioBandPassSetCenterHz`

Needed for:

- `examples/animation-example.rs`
- `examples/audio-graph-example.rs`

## Follow-up

1. Update status/docs that still describe `Action.*` as the primary keyframe authoring model.
2. Add direct live-handle methods for topology, raycast, color, and audio scheduling.
3. Migrate Rust examples that still build `ActionComponent` manually.
