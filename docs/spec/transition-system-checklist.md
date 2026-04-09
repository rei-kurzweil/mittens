# TransitionSystem — phased checklist

Focused implementation checklist for transition/interpolation runtime work.
See [docs/spec/animation-keyframe-interpolation.md](docs/spec/animation-keyframe-interpolation.md) for the design rationale and target semantics.

---

## Scope

This checklist is for the runtime feature that:

- lets components such as `Transform`, `Color`, `Opacity`, and `UV` carry default transition policy
- lets animation actions optionally override that policy for one specific transition
- converts one high-level mutation into per-frame micro-updates via a standalone `TransitionSystem`

This checklist is intentionally phased so we can land useful slices without blocking on the full final design.

---

## Phase 0 — Data scaffold ✅ DONE

Goal: make transition policy authorable in ECS/MMS before any runtime interpolation exists.

- [x] Add `TransitionComponent` data component
- [x] Add `TransitionEasing` enum
- [x] Add `TransitionReplacePolicy` enum
- [x] Add encode/decode support for `TransitionComponent`
- [x] Register `TransitionComponent` in component exports
- [x] Register `TransitionComponent` in `ComponentCodec`
- [x] Register `Transition` in MMS component registry
- [x] Add MMS builder calls for transition basics (`duration_beats`, easing methods, `on/off`, etc.)
- [x] Update interpolation spec for target-component defaults and action override precedence

Acceptance:

- `Transition {}` can be authored under `Transform` today
- `cargo check` passes
- no runtime behavior changes yet

---

## Phase 1 — Transform-only component-default transitions

Goal: get the first end-to-end runtime interpolation working for `UpdateTransform` when the target `TransformComponent` has a child `TransitionComponent`.

### Runtime state

- [ ] Add an internal `ActiveTransition` runtime record
- [ ] Add a `TransitionChannel` shape for transform channels (start with whole-TRS as one unit)
- [ ] Add storage owned by a new standalone `TransitionSystem`
- [ ] Add deterministic replacement keying by target component + channel

### Mutation interception

- [ ] Choose interception point for `UpdateTransform` (preferred: canonical mutation path, not animation-specific code)
- [ ] Detect whether the target transform has a child `TransitionComponent`
- [ ] If no transition applies, preserve existing immediate mutation behavior
- [ ] If transition applies, create/replace an `ActiveTransition` instead of immediately applying the destination transform

### Sampling / playback

- [ ] Snapshot current transform as transition source
- [ ] Store destination transform from intercepted `UpdateTransform`
- [ ] Evaluate progress each frame from duration + timing context
- [ ] Apply easing
- [ ] Emit one `IntentValue::UpdateTransform` per active transition per frame
- [ ] On completion, emit the exact destination transform and retire the record

### Semantics

- [ ] Support `capture_from_current = true`
- [ ] Support `duration_beats = 0.0` as immediate completion
- [ ] Implement `ReplaceSameTarget`
- [ ] If `AllowParallel` is not implemented yet, document/log that it falls back to replacement behavior

### Validation

- [ ] Add a small example or test scene with `Transform` + `Transition`
- [ ] Add tests for replacement behavior
- [ ] Add tests for final-value snap on completion
- [ ] Add tests for “no transition child => immediate behavior unchanged”

Acceptance:

- a transform with a child `TransitionComponent` eases an incoming `UpdateTransform`
- transitions can overlap across multiple different components at once
- exact final transform is reached on completion

---

## Phase 2 — Timing / ownership cleanup

Goal: make transition lifetime and ownership deterministic enough for animation-driven use.

- [ ] Add owner/source metadata to `ActiveTransition` (at minimum enough to identify animation-owned transitions later)
- [ ] Define how beat/time context is passed into transform transitions started from animation
- [ ] Define fallback timing context for non-animation callers
- [ ] Ensure looping/restart can clear or replace owned transitions cleanly
- [ ] Document or implement exact frame-order semantics when other systems also write transform in the same frame

Acceptance:

- transition lifetime is deterministic across restart/replacement cases
- animation-owned transitions can be cleared without affecting unrelated callers

---

## Phase 3 — Action override support

Goal: let animation actions override component defaults without moving transition ownership onto `ActionComponent`.

- [ ] Decide representation for action-local transition override metadata
- [ ] Extend `ActionComponent` encoding/decoding if needed
- [ ] Teach animation-triggered mutations to carry optional override metadata
- [ ] Implement precedence: action override → target component default → none
- [ ] Support explicit “disable transition for this action” override if we want snap-on-this-keyframe behavior
- [ ] Add tests covering override precedence and override-disable behavior

Acceptance:

- a transform can have a default transition policy
- one specific action can temporarily override duration/easing or disable transition entirely

---

## Phase 4 — Additional visual channels

Goal: extend the same runtime model beyond transform.

### Color

- [ ] Intercept `SetColor`
- [ ] Resolve `ColorComponent` child `TransitionComponent`
- [ ] Add color channel interpolation record/value shape
- [ ] Emit one `SetColor` micro-update per frame while active

### Opacity

- [ ] Intercept opacity mutation path (or introduce canonical `SetOpacity` if needed)
- [ ] Resolve `OpacityComponent` child `TransitionComponent`
- [ ] Emit per-frame opacity updates

### UV

- [ ] Pick the canonical UV mutation shape for v1
- [ ] Resolve `UVComponent` child `TransitionComponent`
- [ ] Interpolate the chosen UV representation
- [ ] Add tests/examples for authored UV transitions

Acceptance:

- `Color`, `Opacity`, and `UV` can use the same transition architecture as `Transform`
- unsupported mutation kinds are skipped/logged cleanly

---

## Phase 5 — Diagnostics / authoring polish

Goal: make the feature debuggable and usable without guessing.

- [ ] Log when a transition component exists on an unsupported target/channel
- [ ] Log when override metadata requests unsupported behavior
- [ ] Add at least one example scene demonstrating transition on `Transform`
- [ ] Add an example or note for component default vs action override
- [ ] Update any docs that still imply action-owned transitions only

Acceptance:

- authors can tell why a transition did or did not run
- docs and examples match actual behavior

---

## Phase 6 — v2 audio parameter transitions

Goal: reuse the same architecture for numeric audio parameters, while explicitly excluding graph topology/routing changes.

- [ ] Choose the first audio parameter components to support (gain, cutoff, center frequency, pan, etc.)
- [ ] Add channel kinds and interpolation value shapes for those parameters
- [ ] Intercept canonical parameter mutation intents
- [ ] Reuse the same `TransitionSystem` active-record model
- [ ] Keep graph rebuild / routing / topology changes discrete

Acceptance:

- numeric audio parameter changes can transition
- audio graph structure changes remain non-transitioned

---

## Non-goals

- [ ] implicit neighboring-keyframe track inference
- [ ] arbitrary custom easing curves in v1
- [ ] topology interpolation
- [ ] text interpolation semantics
- [ ] audio graph topology interpolation
- [ ] full blend layers / animation layer stack

---

## Recommended order

1. Phase 1 — Transform-only component-default runtime
2. Phase 2 — ownership/timing cleanup
3. Phase 3 — action overrides
4. Phase 4 — color/opacity/uv
5. Phase 5 — diagnostics/examples
6. Phase 6 — audio parameter transitions

This order gets a real working transition path into `src/` quickly, while keeping the architecture aligned with the spec.
