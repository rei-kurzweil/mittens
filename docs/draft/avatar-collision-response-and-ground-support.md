# Proposal: collision participation, response and avatar ground support

Status: first response-builder/participation slice implemented; the broader
query/solver redesign remains a proposal. See [the implemented API](../how_to/avatar-collision-response.md).
`AVC.collision_response` and `Collidable.slide().response` now support `slide` and
`read_only`. The remote demo uses an enabled read-only capsule. Read-only moving
proxies use the existing retained/refitted query index and native contact
observations; no new per-avatar tree or slide/slide body solver was introduced.

The new motion enum, `Collidable.moving()`, `collision_enabled(...)` MMS builder,
`ground_support(...)` settings and `CollisionQueries` service below remain sketches,
not implemented APIs. The unified ground-support query and removal of the old
collision-disabled support cache are still proposed. Written after support-frame
fix `29ab7169`, which committed the prior behavior before redesigning it.

## Intended behavior

A remotely driven avatar has collision geometry and participates in collision
queries. Its pose comes from the owning peer through ordinary typed-channel/MMS
bindings. Local collision response must not correct that avatar's locomotion
root or velocity. Local foot IK still finds support and solves visual leg poses.

This applies equally to scripted characters, replayed motion, and other externally
driven actors. Multiplayer is one caller, not a special collision exemption.
XR tracking-floor calibration remains a separate coordinate/calibration concern.

## Coupling before the first implementation slice

- `AvatarControlComponent.collision_enabled` currently gates automatic capsule
  creation and capsule validity. The demo previously called `collision_disabled()`
  for its remote copy to avoid local response competing with received movement;
  it now uses the implemented independent response setting.
- `CollidableMode` currently has `Static` and `Slide`. The contact system both
  queries surfaces and applies corrections to authorized slide bodies. Before the
  first slice, its retained BVH indexed static surfaces only. It now also indexes
  read-only moving proxies, with ordinary slide/slide behavior unchanged.
- The lower-body solver uses capsule geometry as its ordinary floor reference.
  Collision-disabled rigs instead get a sole-derived query and a remembered
  support height, with additional frame-change invalidation.

These responsibilities should be separate: publishing/querying collision
geometry, computing contacts, applying response, and choosing visual foot goals.
The shared query index already used by lower-body IK is a useful starting point;
this proposal does not require an independent index per avatar or foot.

Relevant sources:
[`avatar_control.rs`](../../src/engine/ecs/component/avatar_control.rs),
[`collidable.rs`](../../src/engine/ecs/component/collidable.rs),
[`avatar_control_system.rs`](../../src/engine/ecs/system/avatar_control_system.rs),
[`static_contact_system.rs`](../../src/engine/ecs/system/static_contact_system.rs),
[`collidable_bvh.rs`](../../src/engine/ecs/system/collidable_bvh.rs), and
[`lower_body/`](../../src/engine/ecs/system/ik/lower_body/).

## Proposed MMS authoring surface

Keep automatic avatar capsule authoring in AVC, but give response its own setting:

```mms
// Proposed API: ordinary local locomotion, with collision response.
AVC.movement_target(local_root).collision_response("slide") {
    // Existing model mapping, tracking targets and WalkingIK configuration.
}

// Proposed API: full collision participation, externally supplied locomotion.
AVC.movement_target(remote_root).collision_response("read_only") {
    // The same kind of capsule and the same ground-support/leg solver.
}
```

Both retain collision participation. `read_only` restricts collision-system
writes; it does not remove the collider, disable contact queries, or prevent MMS
and typed bindings from writing the authoritative pose. It is unrelated to whether
IK is allowed to adjust the avatar's visual bones.

Explicit full opt-out remains a separate setting:

```mms
// Proposed canonical spelling: no collision participation at all.
AVC.collision_enabled(false) {}
```

The existing `collision_disabled()` should remain a full opt-out during migration,
then be removed or deprecated explicitly. Do not silently reinterpret existing
opt-out calls as read-only collision participation. The multiplayer example has
been migrated explicitly to `collision_response("read_only")` instead.

The equivalent lower-level surface could be:

```mms
// Proposed API for manually authored moving collision geometry.
Zone.capsule_y(0.2, 0.8) {
    Collidable.moving().response("read_only") {}
}

// Proposed correcting body.
Zone.capsule_y(0.2, 0.8) {
    Collidable.moving().response("slide").movement_target(actor_root) {}
}

// Existing static scenery authoring can remain unchanged.
Zone.cube([3.0, 0.1, 3.0]) {
    Collidable.static() {}
}
```

`moving` means the geometry follows a changing authored/external transform; it
implies neither velocity integration nor a new dynamic rigid-body simulation.
Keep `Collidable.slide()` as a shorthand for moving geometry with slide response,
if retaining existing authoring is useful. Exact naming is open for discussion;
the separation between participation and response is the required behavior.

An optional, independent support-query policy could be authored on AVC:

```mms
// Proposed options table, with illustrative values in model metres.
AVC.collision_response("read_only").ground_support({
    max_step_up = 0.15
    max_probe_below_feet = 0.30
}) {}
```

These options select eligible support; they do not enable collision response.
Derive sensible defaults from rig calibration and scale them once. The precise
options should follow the native query contract rather than being fixed now.
No new MMS `GroundSupport` component or generic binding component is required.

## Collision response contract

| Setting | Detect/query geometry | Determine contacts involving this body | Collision corrections to this body's root/velocity | Visual IK writes |
| --- | --- | --- | --- | --- |
| Participation enabled, `slide` | Yes | Yes, subject to filters | Allowed on its explicitly resolved movement target | Allowed according to IK ownership |
| Participation enabled, `read_only` | Yes | Yes, subject to the same filters | Forbidden | Allowed according to IK ownership |
| Participation disabled | No | No through this collider | Forbidden | Independent policy; support can still be queried explicitly |

Read-only is a per-body response policy, not a synonym for a trigger or an
instruction to ignore an entire collision pair. An authorized local slide body
may respond to a read-only body if pair filtering permits that interaction; the
read-only body's pose remains untouched. Two read-only bodies can report contacts
without either being corrected. Avatar-avatar blocking should be an explicit
collision-filter choice, not a hidden consequence of network ownership.

The policy must cover every collision-response write: penetration correction,
slide/sweep movement, velocity projection/friction, route fallbacks, and capsule
settling. A missing movement target must never fall back to correcting some
ancestor of a read-only body. Other systems' pose writes remain governed by their
own ownership rules; this is not a universal engine-wide transform lock.

Physics authority and contact detection are different. A local contact with a
remote avatar does not authorize shared-object ownership changes or authoritative
network damage. MMS/gameplay policy decides what to do with observed contacts.
Per-bone hitboxes and weapon networking remain separate work.

## Native API sketch

Separate geometry motion from the response policy rather than adding another
combined `CollidableMode` variant for every combination:

```rust
// CollisionResponse is implemented. Motion/query types remain proposed.
enum ColliderMotion {
    Static,
    Moving,
}

enum CollisionResponse {
    ReadOnly,
    Slide,
}

struct ColliderSettings {
    enabled: bool,
    motion: ColliderMotion,
    response: CollisionResponse,
    // Geometry/owner/filter identity and optional response target are separate.
}

// Shared scene service; query methods borrow an up-to-date geometry view.
impl CollisionQueries<'_> {
    fn contacts_into(&self, collider: ColliderId, out: &mut ContactBuffer);
    fn sweep(&self, request: &SweepRequest) -> Option<SweepHit>;
    fn ground_support(&self, request: &GroundSupportRequest) -> GroundSupportResult;
}
```

Static geometry has no correcting response; reject an incompatible authored
combination rather than silently enabling movement. Moving read-only geometry
requires a valid geometry frame but no response target. Moving slide geometry
also requires a valid response target. Runtime response changes must not recreate
otherwise valid geometry or change its collision identity.

Contact results should identify both participants and describe geometry such as
normal, point, separation/penetration and sweep time when available. Computing
those results must not mutate either participant. The response pass consumes
results and applies corrections only to bodies whose response permits it.
Materialize scripting contact events only for observers that request them. Reuse
existing `ZoneEntered`/`ZoneExited` semantics for overlap observations where they
fit; do not silently relabel those events as physical support/contact events.
Any new physical-contact event surface needs a separate explicit contract.

Internally, consider three independent readiness states:

- Geometry ready: a live, valid collision shape and frame are available for queries.
- Response ready: a correcting body additionally has a valid authorized target.
- Ground support available: this update's support query found an eligible surface.

Foot IK must not wait for response readiness. Audit the current capsule readiness
fields/events and their consumers during migration; do not silently reuse a
response-ready event to mean only that some geometry exists.

## Ground support without the cached floor-height workaround

Use the same read-only query path for local and remote rigs. Construct its finite
search region from current pelvis/hip placement, calibrated leg lengths and sole
offsets, gravity direction, and explicit step/probe limits. Rest-FK sole landmarks
can contribute to calibration; they must not be the sole narrow height band that
rejects the floor when a crouch places them below it.

Query eligible surfaces and return support candidates plus enough information
for the solver to decide whether planting is appropriate. Reachable ground is
not automatically a grounded foot: airborne intent, pose/tracking authority,
allowed step distance, slope and leg reach still constrain the decision. Exclude
self geometry and inappropriate body parts; query filters should explicitly
control whether another avatar can be considered support.

This must work on the first update when the person is already crouching. It must
also work after reparenting, teleporting, changing scale, or removing a support
surface. No received locomotion/root correction is needed to acquire support.
A failed query must remain a legitimate no-support result.

Replace the current special visual-rig `previous_floor` acceptance reference and
its `support_frame` compensation once this query is proven. Do not remove all
legitimate temporal state: walking stance anchors, contact hysteresis and smoothing
can remain. Their lifecycle must be explicit, and current geometry must validate
them. A previous height may help rank fresh candidates, but cannot be the only
location searched or the only plane accepted.

## Shared index and update ordering

The retained BVH now covers static surfaces and read-only moving proxies. Their
existing moving identity is preserved and dirty bounds are refitted, not rebuilt
on each movement. A response enum alone would not have supplied that queryability.
Ordinary correcting movers are still not mutually resolved by a new body solver.
A separately updated moving-proxy tree remains an option if measurements justify
it; the current retained/refitted approach is the first implementation.

Use a shared scene query service over retained static geometry and separately
updated moving proxies, or another measured equivalent. Share triangle/shape data
and scratch storage. Update/refit dirty moving proxies in batches. Begin with
bounded candidate queries needed by existing consumers; this proposal does not
require an unrestricted all-pairs rigid-body solver.

Suggested phase ordering:

1. Apply input/network/MMS authoritative pose writes and propagate transforms.
2. Update dirty collision geometry/proxies and compute requested body contacts.
3. Apply permitted collision responses, then propagate corrected transforms and
   update affected proxies once for dependent consumers.
4. Query ground support and solve visual legs against the resulting geometry view.

Same-frame dependencies must be defined if visual bones also own collision shapes.
A capsule/root support query must not accidentally read half-updated bone hitboxes.
Preserve the existing attachment/mount gates and choose one ordered writer for each
response target or visual joint. Queries borrow immutable views; response/IK writes
remain explicit engine phases.

## First testable implementation slices

1. **Participation versus response.** Add the native response policy and AVC/MMS
   authoring. Build/update remote capsules instead of bypassing them. Keep existing
   local slide behavior. Make readonly geometry discoverable through the shared
   query service, including its required moving proxy path. Prove no response
   writes before changing foot placement.
2. **Unified support acquisition.** Add the bounded ground-support query and use
   it from both avatar response modes. Remove the collision-disabled solver branch
   and floor-height workaround after standing, crouched-start, moving-frame and
   airborne tests pass.
3. **Example migration.** Configure the remote Mitten with read-only response;
   retain typed-channel root/head/hand/blink writes and local IK. Use the same
   support policy for both copies. Keep committed desktop defaults for Dan and
   perform desktop/XR/Tailscale acceptance separately.

The first response builder/participation slice was subsequently approved and
implemented. The remaining query/solver redesign is not implied by that approval. Before
editing oversized modules, update the existing responsibility-split tickets;
separate query/index ownership, response application and avatar capsule authoring
without introducing tiny modules or per-query runtime dispatch.

## Verification and performance acceptance

- Both local and remote avatars have valid collision geometry. Ray/contact/overlap
  queries can detect the remote capsule after it moves. Self-filtering works.
- A read-only capsule can penetrate a test wall while reporting the contact, yet
  its root, local/world pose, velocity and network-bound values remain unchanged
  by collision response. The equivalent local slide capsule is corrected.
- Pair-filter tests cover local-versus-readonly, readonly-versus-readonly, and
  explicitly disabled avatar-avatar blocking; no implicit ownership transfer.
- Ground support works for both modes while standing and starting already crouched,
  through crouch transitions, on elevated floors, after parent/frame changes, and
  at multiple scales/yaws. The wrong stacked floor is not selected.
- Removing/disabling a floor removes support. Airborne actors do not plant solely
  because some ground is in the search region. Unsupported slopes and unreachable
  targets fail consistently; legitimate stance state survives ordinary motion.
- Response-mode switches retain geometry identity. Removing geometry/targets,
  reusing component slots, disabling participation and mounting/unmounting cannot
  redirect stale corrections to another object.
- MMS construction/serialization preserves both participation and response;
  legacy shorthand migration is explicit. Existing desktop/XR avatar, grabbing,
  local collision response and typed-channel delivery regressions remain passing.
- Profile real avatar/contact/IK work for 2 VR, 4 VR, and 2 VR + 2 desktop rigs.
  Report per-update CPU distributions, candidate and narrow-phase counts, index
  update cost and allocations. Reuse static geometry, retain batched hot paths,
  and avoid per-avatar/per-foot BVHs or per-draw allocations. The existing UDP
  traffic benchmark does not measure these scene costs and cannot validate them.

## Decisions to discuss

- `read_only` is now the implemented response spelling. Decide whether to add
  `moving` as a clearer lower-level constructor alias; avoid `dynamic` if it
  implies rigid-body physics.
- Decide default avatar-avatar blocking and support filters separately from
  network authority. Detection should remain available even when blocking is off.
- Decide whether ground-support options belong directly on AVC initially or on
  the existing lower-body configuration surface. A separate component is not
  required merely to expose query settings.
- Choose the moving-proxy index strategy from measured workloads and define
  contact-event demand, phase ordering and query-result lifetime before public APIs
  are finalized.
