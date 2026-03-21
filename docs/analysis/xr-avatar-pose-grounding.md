# XR Avatar Pose Grounding

Date: 2026-03-21

This document examines the problem of correctly grounding a VTuber-style skinned avatar
in a 1st-person VR scene, given the current engine architecture. It exists to clarify
what is broken, what each fix option requires, and where the engine needs new work.

---

## 1. The problem

The current `vr-input` example parents the avatar model root under the `InputXRComponent`'s
driven `TransformComponent`:

```text
InputXRComponent
  TransformComponent (xr_rig)  ← driven to HMD center pose
    [vtuber model_root]         ← avatar hangs here
    CameraXRComponent
    ControllerXRComponent(L) { T }
    ControllerXRComponent(R) { T }
```

The avatar GLTF model has its coordinate origin at the **feet** (Y=0 in the model's local
space). When the avatar root is placed at the HMD-driven transform, the feet end up at
**head height** in world space. Everything above the feet — the body, head, arms — is
above eye level and out of view. The avatar appears to be standing on top of the player's
head rather than below them.

---

## 2. Why the reference space matters

The engine currently uses `openxr::ReferenceSpaceType::LOCAL` (see `openxr_system.rs`,
`try_init_session`). This has a specific semantic:

- Y=0 is approximately **head height at session start** (wherever the player was when
  the runtime initialized the session).
- When the player stands naturally and the session starts, head pose ≈ `(0, 0, 0)`.
- The "floor" is at approximately `(0, -eye_height, 0)` in LOCAL space — but `eye_height`
  is not known to the engine; it varies per player.

The alternative, `openxr::ReferenceSpaceType::STAGE`, places Y=0 at the **physical floor**
and is the standard reference space for room-scale VR. With STAGE:

- Head pose ≈ `(0, ~1.6m, 0)` when standing.
- Floor = Y=0, known and stable.

STAGE is not always available on all runtimes (it requires the runtime to know where the
floor is — either from physical setup or floor calibration). LOCAL is safer to assume.

**Consequence for avatar grounding:**

With LOCAL space, we cannot reliably know the Y offset between head and floor without
either switching to STAGE or adding a height-calibration step. Any static authored Y
offset for the avatar root will only be correct for one specific player height.

---

## 3. What "correct" looks like for a 1st-person VR avatar

For a useful 1st-person VTuber avatar experience, the avatar should satisfy:

| Goal | Notes |
|------|-------|
| Avatar feet are near the virtual floor, not the head | Grounding |
| Head/neck bone follows HMD orientation | Natural head turn |
| Wrist bones follow controllers | Already working via splice |
| Avatar body doesn't clip into the camera | 1st-person visibility |
| Avatar moves with the player (room-scale) | Optional but desirable |

The current setup satisfies none of the first three.

---

## 4. What we currently have to work with

### 4.1 `InputXRComponent` → driven `TransformComponent`

`OpenXRSystem` drives a direct `TransformComponent` child of `InputXRComponent` to the
HMD **center pose** (averaged from both eye views). This gives us a 6-DOF world transform
representing the player's head position and orientation.

### 4.2 `ControllerXRComponent` → driven `TransformComponent`

Already drives controller pose to a direct `TransformComponent` child. This is used via
the wrist splice (`attach_controller_parent_to_named_wrist`).

### 4.3 Armature splice (`attach_controller_parent_to_named_wrist`)

Splices a `ControllerXRComponent` subtree **above a named bone** in the imported armature.
The arm subtree becomes a child of the controller-driven transform (optionally through a
`TransformPipeline` for filtering). This is the working pattern for hands.

### 4.4 `TransformPipeline` operators (fork / map / filter / merge)

The existing transform pipeline can:
- Fork a transform stream into T/R/S components
- Map each component through an operator (e.g. `QuatTemporalFilter` for rotation smoothing)
- Merge back into a single transform

What it **cannot** currently do:
- Strip or zero individual axes of translation (e.g. "remove Y, keep X/Z")
- Clamp Y to a fixed floor value
- Blend between two pose sources

### 4.5 `TransformPipelineOutput` → subtree adoption

The output leaf of a pipeline can adopt an existing subtree (wrist bone, etc.) as its
child. This is how the splice pattern works.

### 4.6 `find_component` / armature querying

`Universe::find_component(root, selector)` supports CSS-like name selectors and can find
named bones by name. This is how `attach_controller_parent_to_named_wrist` locates wrist
joints. The same mechanism is available for any other named bone (neck, head, spine, etc.).

---

## 5. Approach options

### Option A: Static Y offset under the driven T (partial fix only)

**Topology:**
```text
InputXRComponent
  TransformComponent (xr_rig)  ← HMD center pose
    TransformComponent.with_position(0, -avatar_eye_height, 0)  ← authored offset
      [vtuber model_root]                                         ← feet ≈ floor
    CameraXRComponent
    ControllerXRComponent(L/R) { T }
```

**How it works:** Author a fixed negative Y offset between the HMD-driven T and the avatar
root. When the player stands at their normal height, the avatar's feet are approximately
at the virtual floor.

**What it requires:**
- Knowing `avatar_eye_height` — the Y position of the eye/head bone in the model's local
  armature space. This can be queried at spawn time by finding the head bone's world
  position after GLTF spawn.
- Manually authored or code-queried offset per model.

**Problems:**
- The entire avatar still follows the HMD in all axes including Y. If the player crouches
  physically, the avatar crouches in world space too (feet lift off the virtual floor).
- The avatar body doesn't actually respond to the HMD rotation for the head — the head
  bone stays in its GLTF-posed orientation; only translation follows.
- For LOCAL reference space: the avatar floats above the virtual floor by exactly
  `HMD_local_Y` (which is ≈0 at startup but changes as the player physically moves
  their head up/down).
- This is the simplest thing to author but is strictly a partial fix, not a real solution.

**Verdict:** Useful as a quick debugging aid to see the avatar in frame. Not suitable
for a real 1st-person experience.

---

### Option B: Avatar at fixed world position, head bone splice (no room-scale)

**Topology:**
```text
[world root]
  TransformComponent (avatar_root, at authored floor position)
    [vtuber model_root]  ← stays fixed in world
    ...wrist splices (existing)...

InputXRComponent
  TransformComponent (xr_rig)  ← HMD center pose
    CameraXRComponent
    ControllerXRComponent(L/R) { T }
```

**How it works:** The avatar is NOT parented under the HMD-driven T. It lives at a fixed
authored world position. The HMD-driven T is used only for:
- Eye camera rendering
- Controller wrist-bone driving (spliced into the avatar's armature)
- Head bone driving (spliced into the avatar's neck/head joint, see below)

The avatar stays planted at world Y=0 (or whatever authored height).

**Head bone splice:**
Using the same `attach_controller_parent_to_named_wrist` pattern, splice an
`InputXRComponent`-like pose source above the head/neck bone. This is conceptually
identical to the wrist splice but targeting `J_Bip_Head` or `J_Bip_Neck1`:

```text
J_Bip_C_Spine2
  [HmdPoseSource]  ← new: a pose source that drives HMD orientation into the head bone
    Transform (driven to HMD pose or just HMD rotation)
      J_Bip_C_Head
        ... existing head subtree ...
```

**What this needs:**
- A pose source component that drives ROTATION ONLY from the HMD (ignoring translation),
  so the head bone turns with the HMD but doesn't translate to the HMD world position.
  Currently `InputXRComponent` drives full 6-DOF pose. We need either:
  - A new component (e.g. `InputXRRotationComponent`) that drives only orientation, or
  - A `TransformPipeline` with a `TransformMapRotation` that captures HMD rotation and
    zeroes translation (requires a "zero translation" pipeline operator, currently absent).

**Problems:**
- No room-scale: if the player physically walks left, the avatar doesn't follow.
- The head bone splice with full 6-DOF would drag the head bone to the HMD world position,
  disconnecting it from the spine — body IK would be needed to compensate.
- With rotation-only splice, the avatar's head height is fixed by the GLTF pose, which
  may not match the player's actual head height.

**Verdict:** Best near-term option for a "planted avatar" VTuber scene. Avatar stands
naturally, head turns, hands track controllers. Needs either a new rotation-only pose
source component OR a "zero translation" pipeline operator.

---

### Option C: Floor-anchored avatar, head bone splice (room-scale)

**Topology:**
```text
[world root]
  TransformComponent (floor_anchor)  ← follows HMD X/Z only, Y=0
    [vtuber model_root]              ← avatar at virtual floor, moves with player
    ...wrist splices...
    head bone splice

InputXRComponent
  TransformComponent (xr_rig)  ← full 6-DOF HMD pose
    CameraXRComponent
    ControllerXRComponent(L/R) { T }
```

**How it works:** A `floor_anchor` transform tracks the player's horizontal position
(X/Z from the HMD-driven T) but stays pinned at Y=0. The avatar is parented under
`floor_anchor` so it slides with the player's room-scale movement but never lifts off
the virtual floor.

**What this needs (in addition to Option B's requirements):**

1. **Y-strip translation operator**: A `TransformPipeline` operator (e.g.
   `TransformProjectXZ` or `TransformZeroY`) that takes a transform input and outputs
   only the X/Z translation components, zeroing Y. This does not exist yet.

2. **A way to feed the HMD-driven T into the floor_anchor's pipeline**: Currently the
   `TransformPipeline` only processes data within its own component subtree. There's no
   "read another component's pose as input" concept. `floor_anchor` would need to somehow
   subscribe to the xr_rig's transform stream.

   The current pipeline model is: `source T → fork → map → merge → output leaf → child`.
   The "source T" is always the pipeline's direct parent transform. To drive `floor_anchor`
   from `xr_rig`, we'd need either:
   - Parent `floor_anchor` under `xr_rig` (then the Y-strip operator gives the floor anchor
     its XZ-only local position), but this makes floor_anchor a child of xr_rig, which
     means the avatar is parented under the HMD-driven T — partially defeating the purpose.
   - OR: a new "external pose input" pipeline concept where the pipeline reads from a
     named/referenced transform rather than its own parent.

**Alternative sub-approach for floor anchor (simpler):**

If `xr_rig` is a child of `floor_anchor` (inverted parent-child):

```text
TransformComponent (floor_anchor, Y=0, X/Z authored)
  InputXRComponent
    TransformComponent (xr_rig)  ← HMD pose RELATIVE to floor_anchor
```

Then `floor_anchor`'s position is separately driven by stripping Y from `xr_rig`'s world
position. But this creates a circular dependency: `floor_anchor` depends on `xr_rig`'s
world position, and `xr_rig`'s world position depends on `floor_anchor`.

This is not solvable with the current single-pass transform propagation. It would need an
iterative solver or an explicit "previous frame" position latch.

**Verdict:** Proper room-scale grounding is blocked on:
- A Y-strip pipeline operator (straightforward to add)
- A way to drive a transform from a sibling/external transform source (non-trivial arch work)

Not achievable cleanly in the current tick without new engine primitives.

---

### Option D: STAGE reference space + Option B (cleanest, when available)

Switch the OpenXR session to `STAGE` reference space. Then:

- HMD center pose has Y ≈ 1.6m when standing (real floor = Y=0 in stage space)
- Author the virtual floor at world Y=0
- Avatar root at world Y=0 → feet on floor automatically
- Static authored Y offset = -stage_head_height (≈ -1.6m) to position avatar under HMD,
  OR simply keep avatar at world root and do head bone + wrist bone splices (Option B)

STAGE makes Option B much cleaner because the floor relationship is stable and
runtime-provided. It doesn't fix the problem of head translation vs. rotation on the
head bone splice, but it at least gives a stable floor Y reference.

**Blocker:** `STAGE` is not available on all runtimes. Monado may need a floor calibration
gesture. ALVR and SteamVR generally support it. Using STAGE as a preference with LOCAL as
fallback is feasible, but the fallback still has the unknown floor Y problem.

---

## 6. What the engine needs

### Near term (unblocks Option B, partial Option A)

| Need | Description | Difficulty |
|------|-------------|------------|
| Rotation-only HMD source | `InputXRComponent` variant (or flag) that drives only orientation, zeroing translation on the driven T | Low — one-line filter in `apply_poses` |
| "Zero translation" pipeline operator | `TransformZeroTranslation` (or `TransformProjectXZ`) operator in the pipeline DSL | Low — trivial map operator |
| Head bone splice helper | `attach_hmd_rotation_to_named_head(...)` mirror of `attach_controller_parent_to_named_wrist` | Low — code re-use |
| STAGE reference space support | Try STAGE first, fall back to LOCAL | Medium — affects pose interpretation everywhere |

### Medium term (enables Option C)

| Need | Description | Difficulty |
|------|-------------|------------|
| External pose input for pipelines | Pipeline can read from a named/referenced component's transform as its source, not just its parent T | Medium-High — new pipeline concept |
| OR: Y-strip driven by previous frame | Floor anchor reads last-frame X/Z from xr_rig with 1-frame lag | Medium — acceptable for floor anchor |

### Long term

| Need | Description |
|------|-------------|
| Body IK | Full body solver from HMD + controllers (optional feet trackers). Needed for head splice without body disconnection. |
| Height calibration intent | Player-initiated "calibrate standing height" gesture that sets a per-session `eye_height` value |

---

## 7. Recommended near-term approach

For the current `vr-input` example, the pragmatic path is:

**Step 1**: Move the avatar root OUT from under the HMD-driven T. Give it a fixed world
position (authored at `(0, 0, 0)` or wherever the virtual floor is).

**Step 2**: Keep wrist splices as-is (they work in world space regardless of avatar root
position — see the audit notes on how controller local positions are computed from world
space).

**Step 3**: Splice a rotation-only HMD source above the neck/head bone. This requires
a new `InputXRRotationOnly` flag or component (trivial to add). The rotation drive gives
the head natural turning without pulling it out of the spine chain.

**Step 4**: Accept that with LOCAL reference space, the head bone's world-space Y won't
match the player's physical head height. The avatar stands at its authored pose height.
The head rotates correctly, but height mismatch is not solved without STAGE or calibration.

**Step 5 (optional)**: Try STAGE reference space and document whether it's reliable enough
across the target runtimes (SteamVR / Monado / ALVR).

---

## 8. How the MMS should eventually look

The intended MMS topology for a grounded avatar:

```text
// Floor-level avatar (not under HMD rig)
T.with_position(0.0, 0.0, 0.0) {
    GLTF.new("assets/models/pc-rei.hoodie.glb") {
        EM.on()
    }
}

// XR rig — camera, controllers, head splice source
InputXR {
    T {
        CXR {}
        CTLXR.new(true, Left, Grip) {
            T {
                // (pipeline or direct) → spliced above J_Bip_L_Hand via query
            }
        }
        CTLXR.new(true, Right, Grip) {
            T {
                // → spliced above J_Bip_R_Hand
            }
        }
        // HMD rotation source → spliced above J_Bip_C_Head via query
        InputXR.rotation_only() {
            T {}
            // → spliced above J_Bip_C_Head
        }
    }
}

XR.on()
```

The avatar query + splice is a post-spawn operation (same as the current wrist splice
pattern). The MMS itself expresses the component tree; the splice wiring is a startup
code step.

---

## 9. Current state reference

| File | What it does |
|------|--------------|
| `src/engine/ecs/component/input_xr.rs` | `InputXRComponent` — marker that drives child T from HMD center (full 6-DOF) |
| `src/engine/ecs/system/openxr_system.rs` | `apply_poses()` — drives InputXR T + all ControllerXR Ts from XR pose cache |
| `examples/vr-input.rs` | Current stopgap: avatar under xr_rig, wrist splices post-spawn |
| `examples/vr-input.mms` | MMS mirror of vr-input.rs (avatar also under driven T) |
| `docs/analysis/vr-input-controllerxr-armature-splice.md` | Wrist splice topology detail |
| `docs/analysis/vr-controller-rotation-filter-ab.md` | Rotation filter on controller path |
