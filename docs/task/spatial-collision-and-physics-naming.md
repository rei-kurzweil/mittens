# Task: spatial, collision, and physics naming

## Status and outcome

Design decision, 2026-09-08. Establish names that keep invisible interaction
regions, physical collision participation, spatial shapes, pose constraints,
velocity, and future physics bodies distinct.

This naming applies to
[interaction zones](interaction-zone-collision-query-foundation.md),
[collision-response retirement](retire-collision-response-to-static-nonpenetration.md),
and [pluggable physics](velocity-forces-and-pluggable-physics.md). Exact migration
and compatibility aliases remain implementation work.

## Central distinction

`Zone` is the primitive spatial-region concept. `Collidable` is a capability or
consumer role applied to a zone.

- A **zone** is an invisible, transformable region with identity and a shape.
  By itself it only supports spatial queries and discovery.
- A **collidable** marks a zone for physical contact detection or constraints.
  It may be static, pose-driven, or owned by a physics backend.
- A **spring collider** marks or references a zone as an exclusion region for a
  secondary-motion solver. It does not need to enter physical collision.
- An **interaction role** gives a zone authored meaning such as mount legs,
  torso exclusion, mouth, inventory, damage, or audio ambience.
- A **shape** is geometry used by the zone. It carries no behavior by itself.

Therefore:

```text
every collidable       -> has/resolves a zone
every spring collider  -> has/resolves one or more zones
arbitrary script zone  -> may have neither capability
```

A bare zone does not feed contact consequences to the physics engine. The
`Collidable` role does that. One region may have multiple consumers when sharing
geometry is intentional; authors can use separate overlapping zones when the
shapes, filtering, lifetime, or diagnostic meaning differ.

## Proposed public vocabulary

| Current/provisional name | Target meaning | Decision |
| --- | --- | --- |
| `ZoneComponent` | Primitive invisible spatial region: frame, shape, identity, roles | Add as the common ECS/runtime representation |
| `CollisionComponent` | Zone participates in physical contact detection | Rename toward `CollidableComponent` / MMS `Collidable`; make it a zone role |
| `CollisionShape` | Reusable box/sphere/capsule geometry | Move/generalize as the shape value stored by a zone; `SpatialShape` or `Shape3D` spelling remains open |
| `CollisionShapeComponent` | Separate authored geometry node | Fold into `Zone` for new authoring; retain only as migration compatibility if needed |
| `CollisionMode::Static` | Immovable contact geometry | Replace with an explicit static collidable/body role |
| `CollisionMode::Kinematic` | Externally pose-driven mover | Rename `PoseDriven`; “kinematic” is backend jargon and currently overloaded |
| `CollisionMode::Rigged` | Non-static collider | Remove/replace; “rigged” incorrectly suggests skeleton/IK ownership |
| `CollisionResponse*` | Push/slide/bounce/private integration | Remove |
| `Collidable.slide()` | Prevent its declared movement target entering static collidables while preserving tangential movement | Proposed replacement for authored `CollisionResponse.slide()`; query and response stay separate internally |
| `SpringColliderComponent` | Target-referenced secondary-motion spheres | Migrate to zone instances carrying/referenced by a spring-exclusion role |
| `VelocityComponent` | Linear motion state/command with explicit authority | Add through velocity work |
| `AngularVelocityComponent` | Angular motion state/command | Add through velocity work |
| `PhysicsBodyComponent` | Opt-in simulated body and backend-independent policy | Reserve for pluggable physics |

`Collider` is common terminology, but this codebase already calls component
instances things such as `Grabbable`. `Collidable` describes capability and
avoids implying that the component itself is only geometry. Finalize
`Collidable` versus `Collider` before the migration; do not add both as distinct
concepts.

## Shape ownership and concise MMS

The zone owns the shape. Consumer roles do not duplicate it:

```mms
T {
    Zone.cube([0.30, 0.45, 0.24]).role("mount_legs") {
        name = "legs"
    }
}

T {
    Zone.cube([30.0, 0.05, 30.0]) {
        Collidable.static() {}
    }
}

T { name = "avatar_movement_root"
    Zone.capsule_y(0.28, 0.62) {
        Collidable.slide() {}
    }
}
```

`Zone.cube(...)` creates one zone component containing a normalized shape value.
It does not create a renderable. `Collidable.static()` is behavior/filtering
metadata consuming the enclosing zone; it does not own another shape.
`Collidable.slide()` opts a moving zone into static-contact response. It uses
the zone's nearest transform as its movement target when the zone sits on that
root; generated offset proxies specify `.movement_target(...)` explicitly.
Invisible
is the default, and diagnostic visualization is an editor/system option rather
than an authored visible mesh.

Avoid a shape-less `Zone {}` silently becoming a unit cube. Require a shape
constructor in the first slice. Builder families should remain small:

```mms
Zone.cube([half_x, half_y, half_z]) {}
Zone.sphere(radius) {}
Zone.capsule_y(radius, half_segment) {}
```

Concise compatibility sugar may be retained during migration:

```mms
Collidable.static_cube([30.0, 0.05, 30.0]) {}
```

This must lower to the same runtime zone plus collidable role. Do not keep two
divergent shape representations or require generated hidden ECS children merely
to implement syntax sugar.

## Runtime region representation

The shared system-facing record should be closer to:

```rust,ignore
struct SpatialRegion {
    id: RegionId,
    source_component: ComponentId,
    frame: ResolvedTransformSource,
    shape: SpatialShape,
    enabled: bool,
    roles: RegionRoles,
}
```

`frame` is normally the nearest authored transform, but it may resolve an
explicit target reference. That matters for imported skeletons: current
`SpringCollider.spheres(targets, radius)` expands one configuration into several
target-bound spheres without attaching general collision components to the
imported bones. The zone foundation must preserve that use case.

The spatial registry owns transform-aware world bounds, queries, dirtying, and
optional indexing. Consumers retain their own policy:

- physical collision owns layers, masks, body authority, contacts, and backend
  synchronization;
- secondary motion owns chain selection, hit radius, and exclusion solving;
- attachments own candidate selection, activation, anchors, and handoff;
- scripts own their event/query interpretation.

Do not put all consumer policy into `ZoneComponent` or create a single enormous
zone system.

Possible migrated spring authoring illustrates the same primitive with a
different consumer:

```mms
SpringColliders {
    Zone.sphere(0.11).at("[name='J_Bip_C_Head']") {
        name = "bisket_head_exclusion"
        SpringExclusion {}
    }

    Zone.sphere(0.045).at_each([
        "[name='J_Bip_L_Hand']",
        "[name='J_Bip_R_Hand']"
    ]) {
        name = "bisket_hand_exclusions"
        SpringExclusion {}
    }
}
```

This is provisional migration syntax. Internally, `at_each` expands to stable
region instances sharing authored configuration, like today's
`SpringCollider.spheres`. None of these regions become physical unless they also
carry a `Collidable` role.

## Events and queries

Use **overlap** for geometric intersection without a solved contact. Existing
`CollisionStarted`/`CollisionEnded` events are currently overlap observations,
so `OverlapStarted`/`OverlapEnded` would be more accurate if the event surface
is migrated.

Use zone-specific names when semantic participants are known:

- `ZoneEntered` / `ZoneExited` for continuous observation;
- `classify_point` / `overlaps_shape` / `contains_shape` for synchronous
  queries;
- `ContactStarted` only when a contact/constraint or physics backend actually
  reports contact semantics.

Attachment release uses synchronous zone queries. It must not infer current
eligibility solely from an earlier `ZoneEntered` or generic overlap event.

## Concrete attachment example

Provisional MMS for the broom slice:

```mms
let rider = T {
    name = "bisket_interaction_root"

    T.position(0.0, 0.75, 0.0) {
        name = "bisket_leg_zone"
        Zone.cube([0.30, 0.45, 0.24]).role("mount_legs") {}
    }

    T.position(0.0, 1.35, 0.0) {
        name = "bisket_torso_zone"
        Zone.capsule_y(0.24, 0.32).role("torso_exclusion") {}
    }

    T {
        name = "bisket_broom_rider_anchor"
    }
}

let broom = T.position(2.0, 5.0, -2.0) {
    name = "broom"

    Grabbable.grip("[name='broom_grip']") {}

    Mountable.release_in("[name='bisket_leg_zone']")
        .probe("[name='broom_mount_probe']")
        .exclude("[name='bisket_torso_zone']")
        .rider_anchor("[name='bisket_broom_rider_anchor']")
        .mount_anchor("[name='broom_riding_anchor']") {}

    T { name = "broom_grip" }
    T { name = "broom_mount_probe" }
    T { name = "broom_riding_anchor" }
    GLTF.new("assets/models/broomstick.glb") {}
}
```

This syntax is illustrative, not implemented. It makes the intended component
boundaries concrete:

- `Zone` contains invisible query geometry.
- `Grabbable` owns pickup configuration.
- `Mountable` owns release activation, predicates, participants, and alignment.
- ordinary transforms are probes and attachment anchors.
- the committed attachment relationship is runtime state owned by the
  attachment system.

Dynamic discovery may replace explicit zone selectors with roles:

```mms
Mountable.release_in_owner_zone("mount_legs")
    .exclude_owner_zone("torso_exclusion")
    // anchors and probe as above
```

Here “owner” is the interaction owner resolved from the initiating pointer and
captured when grabbing begins. It is not the world root.

## First delivery order

1. **Zone/query slice:** `ZoneComponent` with embedded cube/sphere/capsule
   shape, MMS constructors, synchronous transformed point classification,
   enumeration beneath a resolved owner, diagnostics, and tests.
2. **Grab relationship slice:** preserve existing pickup behavior while
   publishing explicit active attachment state: pointer/user, held tree,
   follower/target roles, anchors, original basis, and release reason.
3. **Mount handoff slice:** add one `Mountable` release rule in E2, query the leg
   and torso zones, atomically end the grab relationship, and create the reverse
   rider-to-broom relationship.
4. **Dismount/cleanup slice:** restore movement authority, handle removal, and
   repeat the loop before adding flight or generalized proximity mounting.

This order tests zone queries independently, then grabbing independently, then
the direction-reversing handoff. It does not block existing ordinary grabbing
on zones.

## Acceptance criteria

- Authors can distinguish a bare zone from a zone with a physical collidable
  role by component shape and behavior without reading system implementation.
- A simple zone is one invisible component with one embedded shape value.
- Collidables and spring exclusions resolve the same primitive zone records
  while retaining independent physical and solver semantics.
- No public name uses `Rigged` to mean generic movable collision.
- Current overlap events are not described as solved physical contacts.
- The naming record is linked from zone, collision migration, velocity, and
  pluggable-physics tasks.
