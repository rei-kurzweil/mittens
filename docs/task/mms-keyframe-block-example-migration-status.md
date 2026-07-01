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

- `audio_source.play_note(...)` / `audio_source.play_note_at(...)` or
  equivalent direct live-handle audio methods
  - should map to `IntentValue::AudioSchedulePlay`
- `band_pass.set_center_hz(...)`
  - maps to `IntentValue::AudioBandPassSetCenterHz`

Needed for:

- `examples/animation-example.rs`
- `examples/audio-graph-example.rs`

### Known regression after imperative `MusicNote` support

- `Keyframe.at(...) { MusicNote... }` is now audible in live MMS, but the
  current callback-authored audio path appears to bypass the animation
  system's normal audio lookahead scheduling
- this can introduce timing jitter or slight latency, especially on loop wrap
- the current implementation also appears to depend on a janky special case
  where `MusicNote` is parsed like a component expression and then overridden
  to behave like a host method call
- that is not a good long-term runtime model because we do not need persistent
  `MusicNote` instances; we only need host-exposed note-call helpers
- tracked in:
  - `docs/task/keyframe-audio-lookahead-and-musiccontext-removal.md`

### Planned audio API cleanup

- `MusicContext` should be removed from authored MMS for the common direct
  live-handle case
- `MusicNote` should stop being a component entirely
- replace it with a host-provided built-in table:
  - `MusicNote.a`
  - `MusicNote.b`
  - `MusicNote.c`
  - `MusicNote.d`
  - `MusicNote.e`
  - `MusicNote.f`
  - `MusicNote.g`
- each key should resolve directly to a built-in host function rather than
  evaluating additional MMS functions
- this suggests a broader MMS runtime feature: built-in tables for host data
  and host functions, without faking component semantics

## Follow-up

1. Update status/docs that still describe `Action.*` as the primary keyframe authoring model.
2. Add direct live-handle methods for topology, raycast, color, and audio scheduling.
3. Replace `MusicNote` component-expression special casing with a built-in table model.
4. Migrate Rust examples that still build `ActionComponent` manually.
