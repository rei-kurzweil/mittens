# Task: interaction zones on the collision-query foundation

## Status and outcome

First synchronous query slice implemented, 2026-09-08. This task makes the spatial half of
[interaction zones, sockets, and vehicle mounting](release-zones-sockets-and-vehicle-mounting.md)
concrete without coupling zones to the deprecated collision-response runtime.

2026-10-03 direction update: the first slice supports **point classification**,
not general shape overlap or sweeps. The [response/detection retirement task](retire-collision-response-to-static-nonpenetration.md)
now plans to migrate remaining physical contact and the single collision-event
example, then retire the asynchronous `CollisionSystem` worker. Statements
below about preserving the worker/events describe the original first-slice
compatibility boundary, not the final target.

The implemented slice includes the `ZoneComponent` shape constructors and MMS
round-tripping, component-reference-aware `.at(...)`, deterministic subtree and
role discovery, and transform-aware point classification. E2 authoring and the
grabbing/mounting attachment rules remain the next slices.

Add `Zone` as the primitive ECS/runtime spatial-region component. Reuse the
existing `CollisionShape` vocabulary and shared intersection geometry, but do
not model a bare zone as a kinematic body or give it automatic collision
response. Physical collidability, spring exclusion, and interaction meaning are
consumer roles over zones. A bare zone only answers spatial queries.

## Do attachments require zones?

No. Attachment negotiation requires one or more spatial predicates, and an
attachment rule may reference a point, bounds test, socket distance, ray hit, or
another predicate directly. `Zone` earns its place as the reusable authored and
system-facing region:

- scripts and systems can enumerate zones and filter their roles without
  treating every region as a physical contact candidate;
- zones can be named, referenced, enabled, inspected, and visualized;
- collision shapes stay concerned with geometry rather than acquiring mouth,
  legs, seat, inventory, or attachment meaning;
- a bare zone remains non-physical even if the collision response architecture
  changes or a third-party physics backend is selected;
- collidables and spring colliders can share the same transform-aware region
  representation rather than maintaining separate shape registries.

Do not require a zone when a simpler explicit predicate is sufficient. In the
E2 broom case, named leg inclusion and torso exclusion regions are reused by
preview, release validation, diagnostics, and potentially other interactions,
so explicit zones are justified.

## Existing collision foundation

The useful existing pieces are:

- `CollisionShapeComponent` and `CollisionShape` already describe authored
  boxes, spheres, and upright Y capsules.
- `collision_geometry` already contains inclusive shape-pair intersection and
  minimum-translation calculations.
- `CollisionSystem` tracks overlap pairs and emits `CollisionStarted` and
  `CollisionEnded` without requiring `CollisionResponseComponent`.
- collision shapes can be visualized through the existing diagnostic path.

Secondary motion supplies a second existing shape path:
`SpringCollider.sphere(s)` binds target-referenced spheres inside a GLTF
instance, scales each radius from its target transform, and performs its own
sphere exclusion in `SecondaryMotionSystem`. These are not general collision
participants. Their independent representation is evidence that the common
primitive should be a target-bound zone/region, with spring exclusion as a
consumer role.

This means collision detection and collision response are already separable.
Removing response must not remove shapes, overlap queries, or collision events.

The existing collision system is not, unchanged, an adequate zone API:

- `CollisionMode::{Static, Kinematic, Rigged}` describes mechanical roles. A
  detection-only volume has no honest mode in that enum.
- static/static pairs are deliberately skipped. A stationary probe and a
  stationary zone therefore cannot rely on the existing pair matrix.
- collision records carry only a world-space center plus an untransformed
  shape. Parent rotation and scale do not affect box/capsule geometry.
- a `CollisionComponent` participates only as a direct child of a `Transform`.
- worker results are asynchronous start/end observations. A release decision
  must re-query current world transforms and cannot treat a previous preview
  event as authority.
- events identify collider component IDs and a center delta, but do not encode
  zone membership, candidate probes, containment, exclusions, occupancy, or
  attachment-rule priority.

These are reasons to share a geometric/query foundation, not reasons to create
a second independent shape language.

## Proposed authored contract

Initial authoring shape:

```mms
T.position(0.0, 0.75, 0.0) {
    name = "rider_leg_zone"
    Zone.cube([0.30, 0.45, 0.24]).role("mount_legs") {}
}

T.position(0.0, 1.35, 0.0) {
    name = "rider_torso_exclusion"
    Zone.capsule_y(0.24, 0.32).role("torso_exclusion") {}
}
```

`Zone` is a semantic owner for one spatial volume. Its immediate transform
defines the volume's coordinate frame. The component embeds the same normalized
shape value used by collision detection, so the ordinary case is one component
rather than `Zone + Collision + CollisionShape`. It creates neither a
`Collidable` nor a `Renderable`. Require a shape constructor; a shape-less
`Zone {}` must not silently become a unit cube.

The first slice supports a point probe against a zone. The probe is an ordinary
named transform on the candidate tree, such as `broom_mount_probe`. It does not
need to be registered as a physical collider. Later rules may request
shape-overlap or full-containment tests using an authored candidate shape.

`Zone` owns only reusable spatial facts and optional diagnostic presentation:

- enabled state;
- local shape and world-space placement;
- point containment and, later, shape overlap/containment;
- stable identity for references;
- optional enter/exit/eligible preview publication.

It does not own activation gestures, accepted capabilities, attachment
direction, anchors, priority, occupancy, or the action performed. Those belong
to the `Grabbable`/`Mountable` attachment rule and attachment system.

Likewise, a zone does not own physical body policy or secondary-motion solver
policy. A `Collidable` role supplies physical layers/body participation, while a
spring-exclusion role or chain reference tells secondary motion to consume the
zone. The zone remains the shared frame-plus-shape record.

## Discovery and pointer association

An attachment rule may reference zones explicitly. Explicit references are the
authoritative and cheapest path for known interactions such as the E2 broom.

Rules may also request dynamic zone discovery. Discovery begins from the
initiating pointer's resolved interaction owner: the associated avatar/rider
movement tree or an explicitly configured camera-rig/user root. Enumerate
eligible `Zone` descendants of that owner and filter them by the rule's semantic
role/capability. Do not walk the entire world forest merely because the pointer
and avatar eventually share a scene root.

Structural ancestry is only a resolution aid. The actual resolved owner must be
recorded in active grab/attachment state so later release does not select a
different avatar or a newly inserted ancestor. If no owner can be resolved, a
camera-only pointer may still use explicitly referenced world zones, but it must
not invent body-relative mouth, back, torso, or leg zones.

Zone role/tag representation and the exact owner-boundary component remain to be
settled with the attachment runtime. Enumeration must be deterministic and
cacheable, invalidated by zone attach/detach, enable changes, reference changes,
and relevant owner/topology changes. Release still revalidates the selected
zone's current geometry synchronously.

## Query contract

Provide a synchronous query used both by preview evaluation and commit-time
revalidation:

```rust,ignore
enum ZoneRelation {
    Outside,
    Boundary,
    Inside,
}

fn classify_point(
    world: &World,
    zone: ComponentId,
    point_world: [f32; 3],
) -> Result<ZoneRelation, ZoneQueryError>;
```

Transform the world point into the zone shape's local space using the zone's
current effective world matrix, then test the local shape. This naturally
supports translated, rotated, and non-uniformly scaled box zones without
pretending the shape remains world-axis-aligned. Reject singular bases.

Boundary classification must be explicit so an attachment rule can state that
an inclusion zone accepts its boundary while an exclusion zone wins on its
boundary. The E2 broom rule uses exactly that policy.

The authoritative release path performs this synchronous query after resolving
the current broom probe and avatar zones. Cached enter/exit state is preview
only. If a zone, probe, transform basis, or reference is unavailable, the rule
is ineligible and release falls back to an ordinary drop.

## Relationship to `CollisionSystem`

Refactor shared shape resolution and transform-aware query math into a small
collision/spatial-query module consumed by both `CollisionSystem` and zone
queries. A dedicated `ZoneSystem` is only necessary if continuous enter/exit
observation, indexing, or visualization needs persistent runtime state. Do not
create one merely to answer a synchronous containment query, and do not make
zone behavior depend on collision worker timing.

As collision and secondary motion migrate, their system-specific records should
resolve through the same region registry. This need not happen in the first
attachment slice: adapters can translate current `CollisionComponent` and
`SpringColliderComponent` authoring into runtime zones before public syntax is
migrated.

It is acceptable for the first zone slice to use direct synchronous queries
without broadphase registration: the E2 test has two zones and one active held
probe. If later scenes need many continuously observed zones, register their
world AABBs in a shared broadphase while preserving synchronous narrow-phase
revalidation.

Keep `CollisionStarted`/`CollisionEnded` for collider observations. Zone-specific
enter/exit events, if added, should name the zone and candidate/probe explicitly
rather than laundering them through mechanical collision modes.

## Collision-response retirement boundary

`CollisionResponseComponent` and `CollisionResponseSystem` are deprecated. They
currently combine penetration correction, gravity, friction, bounce, transform
integration, and a private runtime velocity accumulator. Do not extend them for
zones, mounting, broom flight, or new movable-body behavior.

Retirement is a separate migration because AvatarControl and a few examples
still opt into `CollisionResponse.slide()`/`push()`. The migration must:

1. preserve collision shapes, overlap detection, and collision events;
2. move commanded/observed motion state to the agreed first-class
   `VelocityComponent`/velocity-driver contract;
3. decide separately whether player non-penetration needs a small character
   constraint/controller rather than a general bounce solver;
4. migrate or remove response-dependent examples and component registration;
5. delete the private velocity accumulator and then remove the response system,
   intents, serialization surface, and obsolete documentation.

IK, pose solving, spring/secondary motion, and collision detection are outside
that deletion. `Velocity` must not become a new name for collision response: it
owns or observes motion, while any future contact constraint consumes collision
queries and changes that motion under an explicit authority policy.

## First implementation slice

Current progress:

- [x] `ZoneComponent`, normalized shared shapes, MMS constructors/builders, and
  serialization round-tripping.
- [x] `.at(...)` accepts either a live MMS component object (stored as a durable
  GUID reference) or a string query. Unprefixed queries resolve in the zone's
  containing scope; `/` and `../` retain their standard meanings.
- [x] Synchronous point classification for cube, sphere, and Y capsule zones,
  including transformed boundaries and singular-frame rejection.
- [x] Stable enabled-zone enumeration with optional role filtering beneath an
  explicit owner root.
- [ ] Author the E2 avatar zones and broom probe.
- [ ] Connect preview and release-time revalidation to attachment negotiation.

1. Add `ZoneComponent` with enabled state, semantic role, embedded shared shape
   value, serialization, and `cube`/`sphere`/`capsule_y` MMS constructors. Add
   registration only if the first implementation actually needs a persistent
   zone index.
2. Add transform-aware synchronous point classification for cube, sphere, and
   capsule zones, including boundary and singular-transform behavior.
3. Add focused tests for translation, rotation, uniform/non-uniform scale,
   boundary contact, missing shape, removal, and unresolved transform basis.
4. Add deterministic role filtering and owner-scoped enumeration, without
   yet migrating physical collision or secondary-motion authoring.
5. Author Bisket's leg inclusion and torso exclusion zones plus the broom probe
   in E2, with non-raycastable debug visualization.
6. Feed the query into mount eligibility preview and intentional-release
   revalidation. Do not implement this as example-local collision event handlers.

Broadphase optimization, arbitrary mesh zones, candidate bounds containment,
continuous crossing detection, and general enter/exit event authoring are
follow-ups, not prerequisites for the point-probe broom slice.

## Future: bounds-derived and adjacent zones

Some zones can be initialized from the aggregate bounds of the mesh subtree
they describe instead of requiring hand-authored dimensions. A useful future
authoring model could separate two operations:

1. fit the zone shape or selected axes to a referenced subtree's bounds;
2. place that fitted shape adjacent to an explicitly selected face, with an
   authored gap, inset, expansion, or thickness.

This could reduce manual placement for vehicle entry regions, audio regions,
interaction envelopes, and inventory volumes. It must remain optional: an
aggregate bounding box cannot infer which side of a car is semantically the
front, whether a doorway rather than the whole body should define entry, or
whether animated/skinned extremities belong in the region. The author must
supply a frame/axis or semantic face unless the asset has trustworthy metadata.

`LayoutRoot` is relevant because bounds now contribute intrinsic width and
height to layout. It may be useful for arranging a zone beside a bounded visual
in the layout plane, especially for panels and flat spatial UI. It should not
silently become the general 3D zone-placement API, however:

- ordinary layout is primarily width/height and does not inherently provide
  the depth needed for a 3D volume;
- a world AABB loses the mesh's oriented local axes;
- layout direction does not establish an asset's semantic front;
- animated or reloaded bounds need an explicit live-versus-initial sizing
  policy;
- visualization markers and zones must not feed back into the source bounds.

Prefer sharing the existing bounds measurement result with a dedicated
bounds-fit/adjacency constraint. Let `LayoutRoot` consume that same measurement
where its 2D semantics fit, rather than making zone authoring depend on a UI
layout component. A later slice should compare local aggregate bounds, oriented
bounds, and authored reference-frame placement before choosing MMS syntax.

## Acceptance criteria

- Physical colliders and zones use the same internal normalized shape value and
  query math; there is no duplicate box/sphere/capsule schema.
- An ordinary authored zone is one invisible component and does not generate a
  collidable, collision response, collision shape child, or renderable.
- Existing physical and spring collider descriptions can be adapted to the same
  runtime region representation without making spring colliders physical.
- A `Zone` never pushes, bounces, integrates, or otherwise moves either party.
- Rotated and scaled zones classify current world-space points correctly.
- Inclusion and exclusion boundary policies can be expressed deterministically.
- An intentional broom release revalidates current zone membership
  synchronously; stale preview state cannot mount it.
- Removing or disabling a zone makes dependent rules ineligible and leaves
  ordinary collision detection operational.
- No new code depends on `CollisionResponseComponent` or its private velocity.

## Related work

- [Editor Zones panel and visualization migration](editor-zones-panel-visualization-migration.md)
- [Rider + Mountable attachment-system first slice](rider-mountable-attachment-system-first-slice.md)
- [Spatial, collision, and physics naming](spatial-collision-and-physics-naming.md)
- [Retire collision response to static non-penetration](retire-collision-response-to-static-nonpenetration.md)
- [Velocity, forces, and pluggable physics](velocity-forces-and-pluggable-physics.md)
