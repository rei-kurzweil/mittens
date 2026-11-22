# Task: release a held item onto a body mount point

Status: release-triggered equipment implemented for Mitten and TDA Miku.
Managed attachment edges, authored snap alignment, and held-size rules remain
planned.

## Release-triggered equipment slice

Mitten and TDA Miku expose two outer-thigh zones and one back zone. A receiver
authored with `Zone.receive_drops(index)` accepts a held tree whose
`Grabbable.attachment_index(index)` is absent or at most the receiver's index.
The avatar receivers use zero. This eligibility index belongs to the drop
affordance and is separate from the valence of managed attachment edges.

A completed grip release tests registered receivers against enabled zones on
the held tree, or its origin when it has no zones. The closest matching
receiver gets `ZoneDropped`, with component IDs breaking ties. The engine
completes the ordinary release first. The avatar handler attaches the prop
with its release world pose and scale, and a later grab can take it off again.

## Responsibility boundaries

Separate grab acquisition and hold following, release negotiation, and inline
tests in `grabbable_system.rs` as follow-up. Group World component indexes and
their lifecycle tracking together, while keeping direct typed lookups and
topology mutation ordering. The release query must run only for actual grip
releases, with no per-frame zone scans or allocations. Validate grip, detach,
and regrab behavior, candidate arbitration, and mounted props under rig motion
and avatar replacement.

## Performer workflow

After using a handheld panel or prop, the performer moves it into an authored
zone on their controlled body and releases the grip. The item attaches to a
matching body mount point and follows the character, leaving both hands free.
An item miniaturized while held remains at that held size on attachment; an
item with no held-size policy keeps its current size. Merely moving through the
zone must not attach it.

## Attachment contract

This is a lower-valence item attaching to a compatible endpoint on the
user-controlled frame, using the
[attachment valence model](attachment-valence-and-grabbable-unification.md).
Valence belongs to the selected attachment endpoints, not to an entire object
or an assumed humanoid skeleton. A body may offer a waist, chest, shoulder, or
other named mount point; a camera rig or creature can expose equivalent points.

Reuse the existing `ZoneComponent` and `zone_query` spatial-query layer for
release eligibility. Author a body-relative `Zone.cube(...)`,
`Zone.sphere(...)`, or `Zone.capsule_y(...)`; its parent transform or `.at(...)`
frame source places it, and `.role(...)` can distinguish equipment zones from
other uses. `zones_in_subtree` can enumerate enabled zones under the controlled
frame, and `classify_zone_point` can test an authored point on the held item at
release. The query already distinguishes inside, boundary, and outside and
reports disabled or unresolved zones. Choose the item's test point and boundary
policy for this interaction; do not create a second zone component or geometry
query path. `ZoneVisualizationSystem` is available for authoring/debug display.

The zone determines where release is allowed. A distinct oriented destination
mount point determines final position and rotation.
The held item's authored source/contact point aligns with that destination;
its arbitrary root origin must not be snapped directly. At grip release,
recheck current zone geometry and compatibility, choose at most one candidate
deterministically, then transfer ownership from the temporary hand attachment
to one retained attachment edge. An invalid, removed, occupied, or ambiguous
candidate falls back to ordinary release. Crossing a zone during a held gesture
cannot attach until the release event.

Scale is an explicit part of the handoff. Capture the object's visible world
scale at the successful attachment commit. If its held-size transition is in
progress, settle to its configured held size before or during the mount
transition without a visible jump. A normally sized item commits at its
current scale. The retained mounted edge preserves that committed world size
under parent transforms, including nonunit parent scale. It must not invoke
the ordinary release path that expands a miniaturized item. Detach/regrab and
detach/drop need deliberate size restoration rules; choose and document those
with the first implementation rather than inheriting an accidental transform.

This feature must use the common attachment lifecycle proposed in the valence
task: single owner, cycle checks, cleanup on endpoint removal, original parent
and world-pose handling, and no movement-authority handoff for equipment. It
builds on [release zones and mount points](release-zones-sockets-and-vehicle-mounting.md)
without coupling its activation to a tablet-size setting. A plain Grabbable
prop must be eligible for the same body mount workflow.

Existing implementation: [`ZoneComponent`](../../src/engine/ecs/component/zone.rs),
[`zone_query`](../../src/engine/ecs/system/zone_query.rs), and
[`ZoneVisualizationSystem`](../../src/engine/ecs/system/zone_visualization_system.rs).

## Acceptance

1. Releasing a compatible held panel inside a body zone attaches it at the
   authored mount point and frees the grabbing hand. It follows avatar motion
   and stays at held size when miniaturization was configured.
2. Releasing a compatible prop without miniaturization attaches it at its
   existing world size.
3. Releasing outside the zone performs an ordinary drop, including the
   held-size restoration policy if one applies.
4. Zone overlap, mount-point orientation, competing candidates, and invalid
   endpoints have deterministic outcomes with no double parentage, transform
   jump, movement-authority change, or stale attachment after deletion.
