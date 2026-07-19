# Avatar control: automated verification harness  (｡•̀ᴗ-)✧

A headless harness that exercises the head-driven AVC + spine FABRIK stack
through a fixed sequence of poses and prints a diff table against an
un-driven reference avatar.  Use when you've touched
`avatar_control_system.rs`, `ik_system.rs`, or `bone_mapping_system.rs` and
want to see whether bones still land where they should — without putting
the headset on.

## What it is

`examples/bisket-vr-debug.{rs,mms}` — a binary example.  Run with:

```bash
cargo run --release --example bisket-vr-debug
```

It exits after printing the report; no window loop, no Vulkan surface, no
OpenXR runtime needed.

## How it's built

### Scene

Two bisket avatars in one world:

| Slot | Position | Driver | Purpose |
| ---- | -------- | ------ | ------- |
| **REF** | `T.position(-1.2, 0, 0)` | none | GLTF rest pose, never moves.  Baseline. |
| **AVC** | `T.position(+1.2, 0, 0)` | scripted `driven_t` TC | Full AVC + FABRIK spine.  Compared against REF. |

`driven_t` for the AVC avatar is just a `TransformComponent` directly under
the placement T; AVC reads it via `parent_of(avc)`.  No `InputXR`, no
`OpenXR`, no `Input` — the harness mutates `driven_t` directly each pose by
emitting `UpdateTransform`, which is exactly the surface a real
HMD-via-OpenXR or desktop-Input would expose.

### Pose script

A small array of `Pose { name, description, t, rot }` covering:

- `rest` — driven_t at HMD height, identity rotation
- `pitch_up_30` / `pitch_dn_30` — head tilts ±30° around X
- `yaw_right_45` — head yaws 45° around Y
- `lean_forward` — driven_t translates +0.2m on Z (head leans out over toes)
- `crouch` — driven_t drops to 1.10m

Extend `poses` in `main()` to add more.  Each pose runs for
`SETTLE_TICKS_PER_POSE` (= 8) frames so IK + transform propagation
converge before sampling.

### Sampling + diff

After each pose settles, the harness reads world-space `(y, z)` of every
joint in `SPINE_BONES` (hips → spine → chest → upper_chest → neck → head)
on both avatars, plus the AVC's `head_mount` and `driven_t`.  Output looks
like:

```
spine bones — model-local Y/Z (REF vs AVC)
  bone                    ref_y    ref_z      avc_y    avc_z         Δy       Δz
  J_Bip_C_Hips           +1.038   +0.004     +1.070   -0.004      +0.032   -0.008
  J_Bip_C_Spine          +1.090   +0.016     +1.122   -0.016      +0.032   -0.033
  ...
```

`Δy` / `Δz` are `avc - ref`.  X is omitted because the two avatars are
intentionally offset on X — comparing X would be noise.

## Invariants checked

Three structural checks run per pose.  These are deterministic — no
golden file needed — and capture properties FABRIK + AVC must hold:

### 1. Bone-length preservation  (tolerance: 5 mm)

For each adjacent pair of spine joints, compare the AVC chain's
`distance(joint_i, joint_i+1)` to the REF rest distance.  FABRIK is
length-preserving by construction, so any drift means the chain is
folding, scaling, or the FABRIK solver is buggy.

### 2. Monotonic Y, hips → neck  (tolerance: 5 mm)

Each successive spine joint's world Y should be ≥ the previous one's.
A drop means the chain is kinking — usually a sign of FABRIK pulling a
mid-joint up past a parent because the target is unreachable in an awkward
direction.

> **Head is excluded** from this check on purpose.  The head bone *pivot*
> sits ~8 cm below the eye line (`eye_offset.y`), and the FABRIK target is
> `HMD - R(HMD) * eye_offset` — so the head pivot legitimately lands below
> the neck whenever the HMD is at eye height.

### 3. `head_mount` lands at predicted target  (tolerance: 10 mm)

`head_mount.world_pos` should equal
`driven_t.world_pos + R(driven_t.world_rot) * head_target_offset`,
where `head_target_offset = R(rot_y(offset_yaw)) * -eye_offset`.  Confirms
the spine FABRIK actually drove head_mount to the AimConstraint-defined
target, rather than head_mount being grounded by a stale `copy_position`
or the chain failing to reach.

## Reading the output (｡◕‿◕｡)

Per pose you get:

1. Header with the pose name + driven_t world pose.
2. Diff table (Y/Z, REF vs AVC, per spine bone).
3. Invariant lines — each `ok` or `FAIL` with the numeric drift.

A pose-by-pose run currently looks like ~30 lines.  All invariants should
say `ok` for any well-behaved change to AVC/IK code.  When something
breaks, the offending invariant prints which joint and what the drift
magnitude was.

### Common failure modes & what they mean

| Symptom | Probable cause |
| ------- | -------------- |
| `bone_length ... FAIL drift=+0.05` repeated across pose set | Solver pulling joints past their parent — e.g. FABRIK iteration count too low for an unreachable target, or chain order reversed (root vs end_effector swapped). |
| `monotonic_y FAIL at J_Bip_C_Chest` only on extreme pitch | FABRIK extending the chain straight at the target with no pole hint — the spine is curling forward through itself.  Need a pole/midline constraint when we add one. |
| `head_mount ... FAIL drift > 1cm` at rest | `copy_position` accidentally re-enabled on the head AimConstraint, OR `target_position_offset` not flowing into FABRIK, OR `head_target_offset_in_target_local` reading the wrong eye_offset (sample before AVC reparents the camera). |
| Pose `rest` already failing | Almost always an init-order issue — bones queried before GLTF spawn, or AVC's `head_mount` not connecting because head bone wasn't found.  Check the `[AVC]` log lines. |

## Limits to keep in mind

- **No golden file yet.**  We assert *invariants*, not specific numbers,
  so the harness can't catch "head Y drifted by 2mm from last week".
  When we want that, snapshot a JSON of `(pose, bone, [x,y,z])` and add a
  `--record` flag to refresh it.  Until then this is a *property* test,
  not a regression test.
- **Reference avatar shares the world.**  Its bone names collide with the
  AVC's, so the harness partitions by `is_descendant_of(avc_id)`.  If you
  add another avatar to the scene, the lookup needs to get smarter.
- **Only the spine chain is checked.**  Arms, fingers, root-yaw, and the
  hips translate-follow (still WIP) are not exercised here.  Add poses +
  bones for those when they land.
- **The `head_mount` prediction depends on capturing `eye_offset` BEFORE
  AVC init**, because AVC re-parents the camera-wrapper T out from under
  itself during init.  See the comment in `main()` near the
  `head_target_offset_in_target_local` call.

## When to extend it

When you add a behavioral feature to AVC, the easiest way to lock in
"this works" is to:

1. Add a `Pose` that puts the rig in a state where the feature applies
   (e.g. for hips translate-follow: a pose with `driven_t.t.x = 0.5`).
2. Add an invariant that captures what the feature should *guarantee* in
   that state (e.g. "hips world X follows driven_t world X within 1cm").
3. Run the harness — see ok / FAIL.

That way the test set grows as the feature set does, without anyone
having to hand-roll a snapshot every time.  rawr 🍷
