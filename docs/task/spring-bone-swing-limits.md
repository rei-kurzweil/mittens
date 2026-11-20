# Spring bone swing limits and tail wag

Status: first slice implemented 2026-11-20 (unbuilt on the authoring machine;
needs `cargo test --lib secondary_motion` and `cargo test --lib roundtrip_spring`)

## Goal

Let a tail droop a little under gravity without hanging straight down, so it
keeps a canine upward curve, and then add an idle wag that still reads right.

## Slice 1: per-chain `max_swing` (done)

- `SpringBone.max_swing(degrees)` applies to every joint of the chain;
  `SpringJoint.new(...).max_swing(degrees)` sets one explicit joint.
- The solver clamps each bone's direction to a cone around its imported rest
  direction in its parent's simulated frame (`clamp_swing` in
  `secondary_motion_system.rs`), after gravity/inertia/stiffness and before
  colliders. The parent frame is rebuilt the way `apply_rotations` writes it.
- Why rest-relative: spring output persists in joint transforms, so a target
  read from current joint positions would be last frame's output and could not
  stop a droop (see the feedback issue in
  [rest-pose return](secondary-motion-rest-pose-return.md)).
- Applied to the shared Bisket tail chain in
  `assets/components/secondary_motion/bisket.mms` at 20 degrees. mitten loads
  that chain through `bisket_shirt_physics`, so every Bisket and mitten instance
  gets it.
- Tests: `swing_limit_holds_a_tail_inside_its_cone_under_strong_gravity`,
  `swing_clamp_leaves_inside_directions_and_rotates_outside_ones_to_the_cone`,
  `chain_and_joint_swing_limits_bind_per_joint`, and the `max_swing` assertion
  in `roundtrip_spring_bone_from_root_preserves_chain_tuning`.

## Next

- [ ] Tune the tail in VR (mitten and Bisket). The limit is local per joint, so
      five joints at 20 degrees allow up to about 100 degrees of total bend.
- [ ] Per-joint overrides on a `from_root` chain: let a `SpringJoint` child
      override only its joint instead of switching the chain to an explicit
      list.
- [ ] Idle tail wag animation. Today the spring output is written relative to
      the imported rest rotation, and the limit is rest-relative, so a wag
      wider than `max_swing` is clamped. Making the limit follow an animated
      primary pose depends on the primary-pose snapshot in
      [rest-pose return](secondary-motion-rest-pose-return.md) phase 2.
- [ ] Do not combine the tail with `ReturnToRestWhenStill`; it would freeze a
      wag at rest.
- [ ] Optional: gravity-relative limits ("never below horizontal"), alongside
      gravity-relative still poses in
      [secondary motion constraints](../draft/secondary-motion-constraints.md).
