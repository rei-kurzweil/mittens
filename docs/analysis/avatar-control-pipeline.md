# Avatar control — pipeline decomposition sketch

Historical note: references below to `TransformPipelineOutputComponent` describe the removed authored output marker. Current authored avatar-control pipelines use `TransformForkTRS` as the root operator node with downstream content attached directly under that fork.

## The problem with the current AvatarControlSystem

`AvatarControlSystem` does two things every tick:

1. Emits `UpdateTransform` on `model_root` — translation + yaw-only rotation derived from `driven_t`.
2. Emits `UpdateTransform` on `head_mount` — full rotation derived from `driven_t`.

This is imperative per-tick math mixed into a system that also does one-time topology init
(bone finding, splice insertion, hand controller re-parenting). The two concerns are conflated.

The per-tick math also has a subtle fragility: `model_root` is a local child of `driven_t`,
so its local translation `(0, -1.6, 0)` gets rotated by `driven_t`'s pitch when the world
matrix cascades. This causes the avatar body to arc in a circle when looking up/down. The
current workaround caches a `model_root_rest_local` field to compensate — correct but awkward.

The deeper issue is: both the circle bug and the need for `model_root_rest_local` only
exist because the system is doing transform math imperatively against a live propagated
world matrix. If the transform pipeline absorbed `driven_t`'s transform and shaped it
before it ever propagated to children, neither problem would arise.

---

## What the pipeline approach gives us

If `driven_t`'s transform is consumed by a `TransformPipeline` node placed directly under it,
the pipeline intercepts the input world matrix and decides what each output subtree sees.
No raw world-matrix propagation reaches `model_root` or `head_mount` without going through
a shaped stream.

This means:

- The body branch can strip pitch/roll from the rotation before `model_root` ever inherits it.
  `model_root`'s local `(0, -1.6, 0)` offset is then only ever rotated by a pure-Y body_yaw
  rotation, which leaves the Y component unchanged. **The circle bug cannot occur.**
- The head branch can keep full rotation and route it to `head_mount` independently.
- Temporal state (body yaw follow) lives in `TransformStreamSystem` keyed by stage path,
  consistent with how `QuatTemporalFilter` already works.
- `AvatarControlSystem` is reduced to topology init only: find bones, create splices,
  displace bones. No per-tick math.

---

## Desired topology

```
Input / InputXR
  └── T  (driven_t — written each tick by InputSystem / OpenXRSystem)
        ├── TransformPipeline  ← body branch
        │     TransformForkTRS
        │       MapTranslation: Pass
        │       MapRotation: ExtractYaw → YawFollow { threshold, rate }
        │       MapScale: Pass
        │     MergeTRS
        │     PipelineOutput (inline)
        │       T.with_position(0, -1.6, 0)  ← model_root
        │         GLTF { }
        │           … armature …
        │               neck_parent
        │                 T  ← head_mount (inserted by AvatarControlSystem init)
        │                   J_Bip_C_Neck
        │
        └── TransformPipeline  ← head branch
              TransformForkTRS
                MapTranslation: Drop
                MapRotation: Pass  (+ π handedness correction for VR, or identity for desktop)
                MapScale: Drop
              MergeTRS
              PipelineRoute { target: head_mount }  ← targeted output, not inline
```

Two sibling pipeline nodes under `driven_t`, both reading `ParentWorld` (driven_t's world matrix).

The body branch output **does not contain pitch**, so `model_root`'s local offset is unaffected.
The head branch writes full rotation to `head_mount` wherever it lives in the armature.

---

## New pipeline operators needed

### 1. `QuatExtractYaw` (new `TransformPipelineQuatOp` variant)

Strips pitch and roll from a rotation quaternion, keeping only the Y-axis component.
Equivalent to: project rotation onto the Y axis, reconstruct a pure-Y quaternion.

```rust
TransformPipelineQuatOp::ExtractYaw
```

Used in the body branch to ensure `model_root` never inherits pitch.

---

### 2. `QuatYawFollow { threshold: f32, rate: f32 }` (new temporal `TransformPipelineQuatOp`)

A stateful temporal quat op that implements body-yaw-follow logic:

- Input: current head yaw (from `ExtractYaw` on `driven_t`'s rotation).
- State: `body_yaw` (f32), maintained across ticks inside `TransformStreamSystem`.
- Output: a pure-Y quaternion for `body_yaw`.
- Logic: if `|head_yaw − body_yaw| > threshold`, advance `body_yaw` toward
  `head_yaw ± threshold` at `rate` rad/s.

Temporal state lives alongside `QuatTemporalFilter` state, keyed by stage path.

```rust
TransformPipelineQuatOp::YawFollow { threshold: f32, rate: f32 }
```

This replaces the body-yaw-follow block in `AvatarControlSystem::tick_one` entirely.

---

### 3. `QuatHandednessCorrection` (new `TransformPipelineQuatOp` variant, or bake into YawFollow)

Applies a configurable Y-axis rotation to the output:

```rust
TransformPipelineQuatOp::YawOffset { radians: f32 }
```

Set to `π` for VR (OpenXR -Z forward vs VRM +Z forward), `0.0` for desktop.

Could alternatively be folded into `YawFollow` as a `yaw_offset` field. A standalone op
is slightly more composable.

---

### 4. `TransformPipelineRoute { selector: String }` (new pipeline terminal)

The existing `TransformPipelineOutput` writes to the next `TransformComponent` child inline
in the component tree. `TransformPipelineRoute` instead writes to a TC found by a selector
string, searching from a configured anchor (default: the pipeline's owner component's subtree
upward to the nearest named ancestor, or configurable).

The result: the head branch can live under `driven_t` and still write to `head_mount` which
is deep inside `model_root → GLTF → armature → neck_parent`. No re-parenting needed.

```rust
// authored component:
pub struct TransformPipelineRouteComponent {
    pub selector: String,
    /// Ancestor depth to anchor the search. 0 = search from pipeline owner upward,
    /// positive = skip N TransformComponent ancestors first.
    pub anchor_skip: usize,
}
```

Alternative: extend `TransformPipelineOutputComponent` with an optional `target_selector`
field rather than adding a new component. Lower surface area.

---

## What `AvatarControlSystem` becomes

After this decomposition, `AvatarControlSystem` only needs to run **once** (or lazily until
bones are found):

1. Find `model_root` (first TC child of AVC).
2. Search model_root subtree for `head_bone` by name. Retry if GLTF hasn't spawned yet.
3. Insert `head_mount` (plain TC) as sibling of head_bone, displace head_bone under it.
4. For each hand bone: resolve controller or insert plain TC splice.
5. Ensure `head_mount` has a stable name/identifier so `TransformPipelineRoute` can find it.

No per-tick code. The system can even be removed after init if it has a "run once and unregister" mechanism.

---

## Open questions

**Q: How does the head pipeline find `head_mount` before it has a stable name?**

`AvatarControlSystem` creates `head_mount` dynamically. Two options:
- Give the created TC a synthetic name (e.g. `__head_mount_<avc_id>`) so the pipeline
  can find it by selector.
- Store the `ComponentId` on `AvatarControlComponent` and give `TransformPipelineRoute`
  a way to read it (e.g. via `ComponentId`-based target rather than selector). This works
  in Rust but not in MMS v1 (which cannot pass live IDs as constructor args).
- Have `AvatarControlSystem` init patch the selector string into the pipeline route component
  after creating head_mount. Slightly imperative but scoped to init.

**Q: Is `QuatYawFollow` the right level of abstraction?**

`YawFollow` is specific to the avatar use case. Alternatively it could be expressed as:
- `ExtractYaw` (new) + a future general `ConstrainedFollow` temporal op with axis + threshold + rate.
This may be over-general for now; `YawFollow` as a named op is clear and testable.

**Q: Two sibling pipelines vs. one pipeline with multiple outputs.**

The sketch uses two sibling pipelines — both reading the same `ParentWorld` input.
Alternatively the pipeline runtime could support a single pipeline with two output terminals.
Sibling pipelines are simpler to author and keep each branch independent. Preferred for now.

**Q: Desktop vs. VR difference.**

Currently handled by `forward_plus_z` on `AvatarControlComponent`. In the pipeline version:
- Body branch: `ExtractYaw` is axis-agnostic (always extracts world Y), so `forward_plus_z`
  doesn't matter here.
- Head branch: `YawOffset { radians: π }` is present for VR, absent (or 0.0) for desktop.
This is a more explicit and local authoring choice than a flag on a coordinator component.

**Q: Does `AvatarControlComponent` still exist?**

Possibly reduced to just the bone name fields and the init state (splice IDs, rest offset),
with no per-tick system. Or dissolved entirely if `TransformPipelineRoute` + `QuatYawFollow`
cover the drive side and a lighter "AvatarSplicer" component covers the topology init side.
