# Rei(mu) XR tree stage

## Goal

Create `examples/rei(mu).mms`: an XR-only Rei(mu) scene with a moving two-sided hair-bow ribbon, a mirror, a procedural tree in a raised square garden, and two instances of the reusable Mittens Corp stage.

This document tracks the work as the scene grows. The first implementation pass establishes the scene with the bow and ribbon as imported, so their visible placement can be checked before changing the model.

## Model inspection (2026-09-25)

`assets/models/rei(mu).glb` contains one skin with 142 joints. Its Bisket-style hair joints include `J_Sec_Hair1_01` through `J_Sec_Hair3_14`, with a fourth joint on chain 13. The existing Bisket spring presets and collider setup are the starting point for those chains.

The bow skeleton branches from one `head_bow` joint into two eight-joint chains:

- `head_bow.001` → `head_bow.002` → … → `head_bow.008`
- `head_bow.009` → `head_bow.010` → … → `head_bow.016`

Those 17 bow nodes are in the skin. The original export had the skeletal `head_bow` directly under `Armature.003`, with the solid bow mesh under `Hair`. The ribbon mesh had no skinning data.

**Updated export, 2026-09-25:** the skeletal `head_bow` is now a child of `J_Bip_C_Head`, and the solid `head_bow` mesh is parented to that skeletal joint. The solid bow should therefore follow head movement as a rigid mesh. The visible `head_bow_ribbon` mesh remains a separate child of `Armature.003`, with no `skin` property and no `JOINTS_0` or `WEIGHTS_0` attributes on its 144 vertices. It cannot deform with the two bow chains in this export, and its head-follow behavior still needs visual inspection. Verify the ribbon's armature modifier targets the exported armature and that the ribbon vertices have weights assigned to the bow chain vertex groups before re-exporting. Preserve the prior export for comparison while investigating.

**Second updated export, 2026-09-25:** `head_bow_ribbon` now has `skin: 0`, and its 144 vertices export `JOINTS_0` and `WEIGHTS_0`. Nonzero weights reach the shared `head_bow` and all 16 joints in the two halves. The model is ready for a spring-motion test; visual quality and bind alignment still need XR inspection.

## Work plan

- [x] Export bow-ribbon skinning with weights on both chains. Inspect bind alignment visually in XR.
- [x] Add `assets/components/secondary_motion/rei-mu-bow.mms` with two separate ribbon spring chains and reuse Bisket hair motion. Initial values need visual tuning for lag, settling, stretch, and clipping.
- [x] Extract the Mittens Corp stage deck/platform, upper and lower steps, back wall, and ceiling truss into `assets/components/studio_stage.mms`. Keep the hanging walkway outside it and update `examples/mittens-corp.mms`.
- [x] Add `examples/rei(mu).mms` with two stage instances, the second offset and rotated 90°.
- [x] Build a bounded recursive MMS tree in a square raised garden with soil and four borders.
- [x] Add a mirror with `Mirror.quality(1440)` for a 1440 × 1440 reflection.
- [x] Set a medium-dark gray background, add `star_kawaii_background`, and light the stage.
- [x] Use `assets/models/rei(mu).glb` with XR input/avatar topology and no desktop or 3D scene camera.
- [x] Keep the example XR-only without a desktop window-size setting. Drive AVC mouth opening from the microphone's rolling amplitude with floor, ceiling, and smoothing; do not use volume normalization.
- [x] Use HTC XR eye tracking with direct pupil direction disabled, while retaining blink samples and driving eye-bone direction with `ambient_eye_saccades`.
- [x] Author an explicit `EditorUI` panel list containing only `settings`, so the default editor panels do not appear.
- [ ] Inspect the solid bow and now-skinned ribbon in XR; verify that both follow the head and each ribbon half moves and settles independently.
- [ ] Load the example through the regular MMS launcher, resolve script/runtime errors, and inspect it in XR. Confirm the mirror, both stage placements, garden, and both ribbon halves visually. Recheck Mittens Corp after the stage extraction.

## References and decisions

- `examples/mittens-corp.mms`: source stage geometry and existing 1440-quality mirror; its suspended walkway is separate.
- `examples/vtuber-secondary-motion.mms`: XR avatar and mirror topology.
- `assets/components/secondary_motion/bisket.mms`, `bisket-shirt-physics.mms`, and `spring_bone_presets.mms`: existing Bisket hair motion.
- `assets/components/backgrounds/star_kawaii_background.mms`: requested star field.

The exact tree silhouette, garden dimensions, stage offset, and spring values can be tuned in the implementation pass. Record the final values and any remaining visual issue here.

## First load check (2026-09-25)

Both `examples/rei(mu).mms` and the refactored `examples/mittens-corp.mms` reach `[CLI] Scene loaded` with the current release binary. The app then fails to create a window because this environment has no Wayland compositor. Visual placement, XR tracking, and ribbon behavior still need a run in a graphical XR environment.
