# Task: retire collision response to a static non-penetration constraint

## Status and outcome

In progress, updated 2026-10-04. Synchronous `overlap_zones(world, a, b)` now reports
inclusive transformed box/sphere/capsule intersection and both current frames
and shapes. `contact_zones` adds separation direction and depth for axis-aligned
boxes and uniformly scaled spheres/capsules. `sweep_capsule_floor` reports a
downward crossing of an axis-aligned floor top, even when the end pose misses
the floor. Rotated contact resolution, general sweeps, a collidable role, and
the contact broadphase remain. Queries report disabled, unresolved, singular,
unsupported-frame, and narrow-phase non-convergence errors. The existing point
query remains intact; focused zone query tests pass.

Retire both the general
`CollisionResponseComponent`/`CollisionResponseSystem` and, after its remaining
event consumer is migrated, the always-running `CollisionSystem` overlap worker.
Keep a narrow static-contact constraint for the avatar and other explicitly
grounded movers. General geometric intersection belongs to synchronous zone
queries, with a collidable role selecting the regions that participate in
physical contact.

Remove kinetic response behavior. The retained constraint has no mass, forces,
gravity, friction, restitution, bounce, momentum, or private velocity. It does
not simulate movable bodies and does not resolve movable-versus-movable contact.
It is an intentionally narrow transition architecture that may later be
replaced by a pluggable physics backend.

This task preserves shape math and geometric query capability, not necessarily
the old collision component, worker, or event names. It does not remove zone
queries, IK, animation, transform streams, or secondary motion.

## Consumer audit (2026-10-03)

- `AvatarControlSystem` generates a kinematic capsule with
  `CollisionResponse.slide()` and routes correction to the locomotion target.
  This is the required runtime migration before response can be removed.
- `examples/bisket-desktop-demo.mms` has one authored camera-rig
  `CollisionResponse.slide()`. `examples/collision-perimeter.rs` and
  `examples/gravity-fields.rs` exercise the legacy response modes/gravity;
  the latter also handles `CollisionStarted`. Rewrite or retire these demos.
- The other authored `Collision.static()` instances are largely scene floors,
  walls, and terrain. They need physical contact only where a grounded mover
  uses them; an otherwise decorative floor does not require collision work.
- No other scene or runtime handler found in this audit consumes
  `CollisionStarted`/`CollisionEnded`. The signal parsing/registration surface
  and tests remain, but are not independent evidence that an asynchronous
  overlap worker is needed in production.
- `ZoneComponent` currently supplies transformed point classification and
  enabled/role-filtered subtree enumeration in `zone_query.rs`. It has no
  synchronous shape-vs-zone overlap, cast/sweep, or continuous enter/exit API
  yet. These queries are Rust system APIs, with no general MMS query method.
  Calling it a replacement for general intersection tests today would
  overstate its implementation.

The audit distinguishes **querying an intersection** from **responding to a
contact**. A bare zone answers the former and never moves anything. A floor
zone with a static collidable role may be consumed by a contact constraint;
the role and constraint, not the zone itself, keep the avatar above the floor.

## Current dependency and problem

The current response component combines unrelated responsibilities:

- static penetration correction (`slide`);
- acceleration away from non-static overlaps (`push`);
- gravity integration;
- friction and speed limiting;
- side-wall restitution/bounce;
- a private runtime velocity accumulator;
- transform movement-target resolution.

Removing the system without migrating AvatarControl and the response-dependent
examples would allow those movers to pass through floors and walls.

The response system should not become the foundation for zones, attachments,
broom flight, or future dynamics. Those uses need spatial queries, explicit
motion ownership, and first-class velocity instead.

## Retained contract

Expose the contact policy on the moving zone's `Collidable`; provisional MMS:

```mms
T { name = "avatar_movement_root"
    Zone.capsule_y(0.28, 0.62) {
        Collidable.slide() {}
    }
}
```

When the collider is an offset/generated proxy whose transform is not the
actual movement root, require an explicit target:

```mms
Zone.capsule_y(0.28, 0.62) {
    Collidable.slide().movement_target("[name='avatar_movement_root']") {}
}
```

The exact builder syntax remains subject to the component pass. `slide` is a
contact-response policy on an opted-in collidable, not a new velocity source.
The implementation can keep query and response phases separate internally
without making authors attach an additional constraint component. The
semantic contract is fixed:

- The moving zone supplies the proposed pose after input, animation, attachment,
  or another pose driver has run.
- Only zones explicitly marked as static collidables participate in this
  contact rule. Ordinary zones remain query-only.
- The spatial query reports both participants, their shapes/frames, whether
  they overlap, and enough geometry to resolve them: a separation direction
  and depth for an existing overlap, or a hit fraction/point and surface
  normal for a sweep. A broadphase may narrow candidates but does not define
  the result.
- The slide policy calculates the minimum world-space correction needed to
  leave static geometry and applies that displacement to its movement target.
  The default target is the zone's own nearest transform only when that
  transform is the intended moving root; offset/generated proxies name a target.
- It stores no velocity and performs no free integration.
- It never pushes the static object or another movable object.
- It does not infer bounce, sliding velocity, gravity, friction, or momentum.
- An unresolved explicit movement target results in no correction and a
  diagnostic; it must not move an arbitrary nearby transform.
- Multiple corrections in one frame have deterministic ordering and a bounded
  iteration count. Failure to converge is observable.

For motion into a surface, the contact rule can remove the into-normal part
of the proposed displacement or velocity while leaving its tangential part.
That is the specific behavior previously called `slide`; it does not require
friction or a separate controller component. The initial migration may
preserve discrete MTV correction and capsule/box/sphere geometry to keep
AvatarControl working. The later
gravity-driven floor path needs at least a crossing/sweep check, since an
end-pose overlap can miss the floor entirely. Stairs, slopes, moving platforms,
and step offsets remain separate movement-policy follow-ups.

## Motion and authority

Call the retained object **pose-driven** or **movable**, not kinetic. Its pose is
owned by input, animation, attachment, a velocity driver, or some other named
source. Static contact constrains that proposed pose; it is not a second
integrator.

If a first-class `VelocityComponent` is present, this transitional slide policy
does not integrate it. A later policy may project or zero velocity along a
contact normal, but that belongs to the velocity/physics contract and must be
explicit. The first migration can leave commanded velocity unchanged and only
correct the pose, provided the limitation is documented so a driver does not
silently acquire fake bounce behavior.

Exactly one system owns the movable transform update at a time. Applying the
constraint through the existing world-displacement/movement-target path is
acceptable as an intermediate implementation, but the correction must occur
after the proposed pose and before cameras and dependent interaction queries
consume the final world transform.

## Migration plan

Start visible verification with an AVC capsule against a simple flat floor in
both desktop and XR. `examples/secondary-motion-desktop.mms` is the desktop
candidate: its `studio_floor` has a top at world y=0, and its input-driven AVC
routes capsule corrections to `avatar_head_driver`. Make that floor a static
zone/collidable, and migrate AVC's generated capsule to a slide collidable with
the explicit movement target. Leave its grabbable pile cubes on the legacy path
until horizontal obstacles are in scope. The floor is authored under a scaled
transform, so a zone on that transform uses local half extents `[0.5, 0.5,
0.5]`, not half the already-scaled world size. Once migrated, run
`cargo run --release -- load examples/secondary-motion-desktop.mms`: `F` drives
the desktop avatar downward into the floor, while `WASD` checks that corrected
movement along the floor remains free.

`examples/mittens-corp.mms` is the first XR candidate. Its player is Rei(mu),
with the two bow spring chains and their `ReturnToRestWhenStill` constraints.
The AVC uses `rei_mu_locomotion_root`; the car's Rider and mounted controls
retain that movement root. The shared `studio_stage` deck has top y=0.12 and
lies under Rei(mu)'s initial x/z. Make the deck a static zone/collidable first.
Its lower `studio_floor` is a fallback surface, not the initial standing
surface. Verify that the generated capsule constrains the locomotion root
rather than the visual model or HMD tracking transform, and that entering or
leaving the car keeps the current movement authority. Since this slice adds no
gravity, use an explicit downward pose change or controlled test drop to
demonstrate floor crossing, then check that the capsule bottom remains at or
above the deck top. Headless tests should cover initial penetration, a
downward crossing that ends below a thin floor, and tangential XZ movement
after correction. Visual XR verification requires an XR runtime and headset;
run `cargo run --release -- load examples/mittens-corp.mms` after the XR
scheduling path is in place.

The current tick order needs attention before these are live contact examples:
desktop input settles before the old collision-response phase, but XR gamepad
locomotion runs after the camera and old response phase, and AVC may create its
capsule later in the frame. Place the new constraint after each proposed mover
pose and before the camera and interaction consumers that need the corrected
pose; a late correction followed only by rendering would leave XR views or
queries one pose behind. Keep first-frame capsule creation and subsequent
movement-target updates covered by headless scheduling checks.

Other scenes remain follow-ups: `examples/vtuber-mirror-example.mms` and
`examples/vtuber-slidedeck.mms` have XR avatars and temple floors, but their
visible risers sit above the authored ground plane; choose the actual standing
surface before enabling contact. `examples/capsule-stick-figure.mms` is a
second staged XR candidate. `examples/bisket-vr-demo.mms` uses the
72-by-72-cell voxel terrain and should exercise the static contact index after
the small-floor path works. `examples/e2.mms` has a rotated floor frame, which
the current separation query explicitly rejects; migrate it after rotated
contact geometry or reauthor that floor as an axis-aligned box.

Then use `examples/collision-perimeter.rs` for the first visible horizontal
slide test. It already has an input-driven camera sphere, a static floor, and a
perimeter of walls. Migrate that sphere and those static surfaces to `Zone`
plus `Collidable.slide()` / `Collidable.static()`; keep the camera and perimeter
so walking into and along a wall demonstrates the behavior. Remove its old
`Gravity` field, pushable cubes, and response-on-static-wall demonstration;
those belong to later gravity/dynamic-body work. Add headless movement and
contact checks so correctness does not depend on visual inspection.

Next migrate the single camera-sphere slide in
`examples/bisket-desktop-demo.mms`. The larger
`examples/gravity-fields.rs` depends on old push, private gravity velocity,
and `CollisionStarted` handlers; repurpose it for first-class gravity and
dynamic bodies when those exist, or remove the obsolete interactions during
retirement. Keep the static-only MMS floors where a migrated mover needs
them, and remove collision participation from decorative floors.

1. Add synchronous transformed zone-to-zone shape overlap and a minimal
   capsule-versus-static-floor crossing/sweep query. Specify inclusive
   boundaries, filtering, deterministic hit order, and unresolved/singular
   frame errors. Expose the general point/overlap queries to MMS when a script
   consumer needs them. Reuse `collision_geometry` math rather than creating a
   second shape implementation. Keep point queries intact.
2. Introduce `Collidable.static()` and `Collidable.slide()` on zones, backed by
   a narrow contact query/response phase. Migrate AvatarControl's generated
   capsule and required authored slide users, preserving proxy cleanup and
   movement-target routing. For the later falling-avatar path, contact must
   also remove inward velocity from the first-class `VelocityComponent`.
3. Migrate static floors/walls that actually need contact to zones with a
   static collidable role. Convert detection-only uses to bare zones; remove
   unused authored colliders from decorative geometry. Finish the two Rust
   example migrations and the single MMS response demo as described above.
4. Remove `CollisionResponse.push()`, non-static repulsion, old gravity
   integration, friction, restitution, speed limiting, and private velocity.
   Delete `CollisionResponseComponent`/system, registration intents, MMS API,
   and obsolete response serialization tests/spec.
5. Once the `gravity-fields.rs` event handler is gone and tests use synchronous
   zone queries, remove the asynchronous `CollisionSystem` overlap worker and
   old `CollisionStarted`/`CollisionEnded` API. Add zone enter/exit observation
   only for a concrete consumer; current-contact reports should use contact
   semantics. Preserve shared shape/query math and debug visualization through
   the zone path.
6. Remove or migrate `CollisionComponent`, `CollisionShapeComponent`, and
   `CollisionMode::{Static,Kinematic,Rigged}` after their authored, generated,
   and diagnostic consumers have moved. Do not encode movement authority in
   the collidable role.

## Performance and scheduling

The current collision worker builds a separate BVH from all collision objects
on every worker tick, then sends overlap pairs back for later event dispatch.
That asynchronous design suits enter/exit observation but can give contact
response a pair snapshot from an earlier pose. `BvhSystem` is a different BVH
over raycastable renderables; it refits or rebuilds and answers raycasts on the
simulation thread after transform updates. Its leaves are not physical
collidable zones. Neither existing index is a same-step contact index for the
proposed zone path.

A contact BVH and a contact worker are separate decisions. Keep contact
queries synchronous and use a static-collidable index when the candidate set
justifies one. Do not add a dedicated worker merely because there are many
zones: indexing reduces candidate work, while threading alone does not. If
profiling shows that index maintenance or independent queries exceed the frame
budget, consider worker jobs with versioned immutable snapshots or a same-step
completion barrier.
Corrections must still consume results for the current proposed pose before
cameras and interaction systems read the final transform. Measure the cost of
snapshot transfer, synchronization, and missed frame deadlines before adopting
that design.

Keep a contact broadphase over zones with a `Collidable` role. Cache static
zones' world AABBs and index them until their shape or effective transform
changes. Refit/update moving zones as needed. A sliding mover queries with the
union of its current and proposed world AABBs (a swept AABB), filtered to
eligible static collidables and excluding itself/its own rig. The broadphase
returns candidates; exact transformed shape overlap or sweep tests then
provide hit normals, depths, and times. Process the relevant hits in a stable
order and bound correction iterations. A simple scan may be cheaper for a
scene with only a few floors; add an index when measured candidate work calls
for it.

Reuse BVH data structures or indexing utilities where useful, but do not use
the renderable BVH's contents as the contact set: invisible floors, capsules,
and semantic zones need their own identities and filters. Contact decisions
must use the current proposed pose in the simulation step; the old worker's
asynchronous pair snapshot can lag behind it. Do not rebuild or scan all zone
pairs solely to constrain one avatar. Track dirty transforms and avoid work
for unchanged participants where correctness allows it.

Keep shape resolution and narrow-phase math shared with ordinary collision and
zone queries. Do not fork capsule/box/sphere intersection implementations.

Record at least candidate-pair count, narrow-phase test count, correction
iterations, and non-convergence count so the retained path can be compared with
a future backend.

## Acceptance criteria

- Desktop and XR AvatarControl capsules remain outside static floors and walls.
- Pose-driven objects are not automatically pushed by other movable objects.
- No retained collidable contains velocity, gravity, friction, restitution,
  bounce, force, or mass state.
- `CollisionResponse.push()` and its private velocity accumulator are removed.
- Point and shape zone queries work without a response component or collision
  worker. A collidable role gates physical contact; a bare zone has no effect
  on motion.
- The legacy collision event consumer is migrated or intentionally removed
  before its signal surface and worker are deleted.
- System ordering exposes one final corrected pose to cameras and interaction
  consumers.
- Tests document discrete-correction limitations rather than implying robust
  rigid-body simulation or slope/step behavior.

## Related work

- [Spatial, collision, and physics naming](spatial-collision-and-physics-naming.md)
- [Velocity, forces, and pluggable physics](velocity-forces-and-pluggable-physics.md)
- [Interaction zones on the collision-query foundation](interaction-zone-collision-query-foundation.md)
- [AVC auto-calibrated upright capsule](avc-upright-character-capsule.md)
- [Velocity / AngularVelocity components WIP](wip/velocity-components.md)
