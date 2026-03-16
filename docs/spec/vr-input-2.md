# VR input 2: current XR input wiring

This doc is a follow-up to [docs/spec/vr-input.md](docs/spec/vr-input.md).

It focuses on the current OpenXR pose flow for controllers and hands, and on the point where that XR state becomes normal engine transform data.

Scope:

- current suggested OpenXR interaction profiles
- current hand-tracking behavior
- current unification path from OpenXR into `ControllerXRComponent` and `TransformComponent`
- where future transform filtering should attach

This doc does **not** define button/trigger gameplay semantics yet.

The broader transform-processing design has been split out to [docs/spec/transform-pipeline.md](transform-pipeline.md).

---

## Current state: what the engine does today

At a high level, the engine currently has two OpenXR pose sources that can drive the same authored ECS shape:

- controller pose actions (`aim` / `grip`)
- hand-tracking root pose (currently chosen from `WRIST`, with `PALM` fallback)

Those sources are unified by `OpenXRSystem`, then applied to authored `ControllerXRComponent` nodes by updating a child `TransformComponent`.

Conceptually:

```text
OpenXR runtime
  -> OpenXRSystem::render_xr()
    -> cache hand root poses
    -> cache controller aim/grip poses
  -> OpenXRSystem::tick_with_queue()
    -> choose preferred pose source for each ControllerXRComponent
    -> update TransformComponent child
```

This gives the rest of the engine a normal transform boundary instead of forcing downstream systems to understand raw OpenXR action spaces or hand-joint queries.

---

## 1. Suggesting input profiles

The current OpenXR path still suggests a set of interaction profiles to the runtime.

That is the right default behavior.

Suggested profiles are best understood as:

- hints to the runtime about the kinds of bindings the app knows how to use
- not guarantees that any specific profile is active
- not guarantees that actions are actually bound on a given runtime/device combination

In practice this means:

- `OpenXRSystem` should keep suggesting the controller profiles it knows about
- it should also allow hand-driven behavior when hand tracking is available
- runtime diagnostics remain important, because a focused session can still have unbound action paths

So the engine’s current model should stay:

- suggest supported controller-style profiles
- query what is actually active/bound at runtime
- consume whichever pose source is really available

---

## 2. How we currently handle hand tracking

The current hand-tracking path uses `XR_EXT_hand_tracking` as a raw-joint input source.

At the engine level, we currently reduce that richer data down to a single per-hand root pose.

The current selection policy is:

1. use `WRIST` if its position and orientation are both valid
2. otherwise use `PALM` if its position and orientation are both valid
3. otherwise treat the hand root as unavailable

This gives a pragmatic first step:

- existing controller/hand debug visuals can move with tracked hands
- authored `ControllerXRComponent` nodes can be driven even when controller action poses are absent
- the engine can validate hand-tracking availability without first designing a full hand-skeleton ECS layer

### Meaning of the current hand root

The chosen root pose is **not** the same thing as a controller grip pose.

That distinction matters:

- a controller `grip` pose is an interaction/runtime-defined holding pose
- a hand `palm` or `wrist` pose is anatomical joint data

So current hand-root behavior should be understood as:

- “use a stable tracked hand-root-ish pose to drive a transform”
- not “pretend OpenXR hand tracking already gives us a canonical controller-equivalent grip pose”

---

## 3. Current unification path: OpenXR API -> OpenXRSystem -> ControllerXRComponent -> TransformComponent

This is the core wiring that exists today.

At a high level:

1. `OpenXRSystem` samples OpenXR state each XR frame.
2. It gathers controller action poses when those actions are active and valid.
3. It also gathers hand-joint data when `XR_EXT_hand_tracking` is available.
4. For each hand, it resolves a preferred root pose.
5. `ControllerXRComponent` uses that resolved pose to drive its child `TransformComponent`.

This means the authored scene graph sees a regular transform boundary, not raw OpenXR action-space details.

That unification is important because it keeps downstream authored content simple:

- a hand mesh subtree can hang from the transform
- a debug cube can hang from the transform
- a ray origin helper can hang from the transform
- future filtering can happen at the transform boundary rather than inside XR-specific code

---

## 4. Current pose precedence

The current precedence is intentionally simple:

1. prefer the tracked hand root when hand tracking is available and valid
2. otherwise fall back to the controller action pose
3. otherwise leave the target without a resolved XR pose for that frame

This gives one consistent transform-driving path for:

- controller-backed interaction profiles
- hand-tracking-backed interaction
- future debug or visualization helpers that only care about a resolved transform

The key design point is that this precedence is resolved inside `OpenXRSystem`, while authored content still just consumes a transform.

---

## 5. Relationship to future transform filtering

The current XR path is already cleanly unified at the `TransformComponent` boundary.

That means future smoothing, follow, or secondary-motion behavior should attach **after** XR pose resolution, not inside the raw OpenXR sampling logic.

In other words:

- OpenXR should keep doing source acquisition and basic pose resolution
- transform processing should happen in the general transform-processing layer

That broader design is now documented separately in [docs/spec/transform-pipeline.md](transform-pipeline.md).

That doc covers:

- `TransformPipeline` as the topology-side authored representation
- `TransformPipelineProcessor` as the runtime/system evaluator
- TRS fork / map / merge vocabulary
- quaternion-first rotation smoothing via operators like `QuatTemporalSmooth`
- broader use cases like dynamic bones, hair, cat ears, clothing lag, and secondary motion

The XR-specific takeaway is simple: future smoothing should be layered onto the transform-processing boundary, not hardcoded into `OpenXRSystem`.

---

## 6. Summary

Current engine behavior:

- suggests supported controller-style profiles to the OpenXR runtime
- samples controller `aim` / `grip` poses when available
- samples raw hand tracking via `XR_EXT_hand_tracking`
- reduces hand tracking to a per-hand root pose (`WRIST`, fallback `PALM`)
- unifies both sources inside `OpenXRSystem`
- drives a child `TransformComponent` under each `ControllerXRComponent`

Current architectural takeaway:

- XR input is already unified at the transform boundary
- downstream authored content does not need to care whether the source was a controller action space or a hand-root joint estimate
- future smoothing and follow behavior should build on the general transform pipeline described in [docs/spec/transform-pipeline.md](transform-pipeline.md)
