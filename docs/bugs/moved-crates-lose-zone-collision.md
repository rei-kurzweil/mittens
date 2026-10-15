# Moved crates lose Zone collision

Date: 2026-10-14
Status: reported; source investigation only, interactive reproduction and fix pending.

## Report

In examples containing crates with collidable Zones, moving a crate even slightly
makes it stop colliding with the player. The exact movement method and resulting
rotation have not yet been isolated. Record whether this happens during a grab,
after release, through an editor translation, or through all of these paths.

Crates should retain contact at their new visible position after repositioning.
This is separate from [persistent player drift after crate contact](player-keeps-drifting-after-jumping-onto-crates.md).

## Reproduction fixtures

- [graphics-settings.mms](../../examples/graphics-settings.mms)
- [secondary-motion-desktop.mms](../../examples/secondary-motion-desktop.mms)
- Shared [stage_with_boxes.mms](../../assets/components/stage_with_boxes.mms),
  for example `pile_a_base_left`.

The shared factory puts `Grabbable`, the cube renderable, and
`Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }`
under the same scaled Transform. Zone extents are local to that Transform.

1. Load a fixture and verify side contact and standing on an untouched crate.
2. Move one crate a few centimetres, release it if grabbed, and retry both contacts.
3. Compare pure translation with yaw-only rotation and slight pitch/roll.
   Use a direct translation as a control that does not reparent the crate.
4. Check both the new and old positions. Record the crate's parent, local TRS,
   world matrix, Zone/Collidable enabled state, and player capsule/grounded state
   before movement, while held, and after release.
5. Check whether returning the crate to an upright pose restores collision,
   and whether scene reload restores the original behavior.

The report is not yet reproduced by an automated or interactive investigation.

## Source evidence and likely investigation path

- [StaticContactSystem](../../src/engine/ecs/system/static_contact_system.rs)
  enumerates enabled Collidable children of enabled Zones on each contact pass.
  Its retained `previous_centers` are for moving slide Zones, rather than a
  persistent cache of static crate positions. Do not assume stale static
  registration is the cause.
- [Zone frame resolution](../../src/engine/ecs/system/zone_query.rs) uses the
  nearest ancestor Transform unless an explicit frame source is authored.
  Check that propagation and frame ownership still match the visible crate.
- `StaticContactSystem::surface_world_shape` accepts axis-aligned shapes or an
  upright `yaw_box`. If both conversions fail, that surface is silently skipped.
  `surface_translation` also has an upright-box fallback for moving spheres and
  capsules; it does not implement general tilted-box contact response.
- [Contact geometry](../../src/engine/ecs/system/collision_geometry.rs)
  rejects pitch/roll or non-orthogonal axes in `yaw_box`, using a `1e-6`
  tolerance. General transformed Zone overlap is available, but overlap alone
  does not provide a supported separation/contact response for every frame.
- [GrabbableSystem](../../src/engine/ecs/system/grabbable_system.rs) reparents
  a grabbed Transform beneath the pointer's nearest Transform. On release it
  restores the original parent while preserving the full world pose, including
  rotation, and emits transform updates. An incidental tilt can therefore remain
  after a small apparent move. Also inspect decomposition under scaled parents.

**Hypothesis:** a grab introduces slight pitch/roll, leaving the visible crate
with a contact frame the solver rejects. This is a supported source-level
possibility, not a confirmed diagnosis of the user's translation report.
If pure translation also fails with an upright, non-singular world matrix,
investigate transform propagation/reparenting before expanding contact geometry.

## Fix scope and acceptance

- Isolate translation, reparenting, yaw, and tilt cases before choosing a fix.
- Repositioning a supported upright crate must update contact at its visible
  location and remove contact from its old location.
- Decide how grabbable collidables handle tilt: support the needed oriented
  contact response or define an explicit movement constraint. Silently losing
  collision during ordinary repositioning is the bug, not a usable policy.
- Preserve authored scale, friction, player grounding, and swept landing.
- Cover grab/release as well as direct transform updates in a focused regression,
  including the small rotation or transform change that reproduces this report.

Related: [Mittens-corp jump controls and box stacks](../task/mittens-corp-jump-controls-and-box-stacks.md),
[surface friction](../task/surface-friction-for-static-collidables.md).
