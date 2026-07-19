# Collision-system audit follow-up

Date: 2026-07-19

Status: planned.

- [ ] Decide whether `CollisionMode::Kinematic` should become `Movable`.
- [ ] Specify exact Static, Movable, and Rigged semantics and collision matrices.
- [ ] Define non-penetration, response authority, and displacement sharing for movable pairs.
- [ ] Prevent players entering movable cubes while pushing them.
- [ ] Add swept/continuous detection for fast motion.
- [ ] Audit contact manifolds, resting stability, stairs, slopes, bounce, mass, and authority.
- [ ] Add bounds-to-shape heuristics for boxes, spheres, capsules, skeletons, selected meshes,
      and accessory exclusion.

This audit must not retroactively broaden the AVC capsule task or silently
change its existing velocity, friction, gravity, restitution, and iteration policies.
