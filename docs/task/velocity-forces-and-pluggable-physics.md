# Task: velocity, forces, and pluggable physics

## Status and outcome

Architecture planning, 2026-09-08. Define a motion model that supports simple
engine-owned velocity integration now and can delegate rigid-body simulation to
a third-party physics engine later without changing authored attachment, zone,
or transform semantics.

Do not select a third-party engine in this task. First establish component data,
authority, stepping, synchronization, and backend boundaries that can be tested
with a small built-in integrator and a mock backend.

## Separate the concepts

The current collision-response path mixes detection, contact policy, velocity,
forces, and transform mutation. The replacement architecture keeps these
separate:

- **Zone/shape/query:** primitive spatial regions, geometry, broadphase
  candidates, overlaps, raycasts, and sensors. Physical collision and spring
  exclusion consume zones rather than defining separate region types.
- **Motion state:** linear/angular velocity and optional history.
- **Forces:** accumulated force/torque or acceleration requests with explicit
  lifetime and coordinate space.
- **Integration:** advances a simulated body through time.
- **Constraints/contact:** prevents penetration or establishes joints, mounts,
  and other relationships.
- **Pose synchronization:** transfers authoritative results between component
  transforms and a physics backend.
- **Interaction semantics:** grabbing, zones, attachment negotiation, mounting,
  and control ownership. These must not be defined by a particular backend.

## Proposed core components

Gravity and Acceleration use the selected velocity-driver ancestor topology:
they wrap the Velocity they drive, directly or indirectly. The
[focused driver task](gravity-and-acceleration-velocity-drivers.md) tracks
descendant scope, nested Velocity boundaries, and the next implementation slice.

Names are provisional; responsibilities are not.

### `VelocityComponent`

Stores world-space linear velocity in world units per second. It may be
commanded by script, derived from pose changes, or owned by a simulation
backend. Optional history supports effects and diagnostics but is not required
for integration.

### `AngularVelocityComponent`

Stores world-space angular velocity as an axis-angle vector in radians per
second. Its direction is the axis and its magnitude is angular speed.

### `ForceAccumulatorComponent`

Collects forces and torques for a particular simulation step. Requests specify
world/local space and force/impulse semantics. Transient accumulators clear at a
defined step boundary; persistent forces such as gravity are modeled as named
providers rather than values that accidentally accumulate forever.

### `PhysicsBodyComponent`

Opts a transform into simulation and carries backend-independent body policy:

- authority: pose-driven, velocity-driven, or backend-simulated;
- mass/inverse mass and inertia policy when simulated;
- enabled/sleeping state;
- collision layer/mask and sensor participation;
- continuous/discrete motion request;
- stable backend handle lifecycle.

Static collision geometry need not carry velocity or a simulated body. A bare
zone is queryable only. A `Collidable` role exposes that zone to contact or a
physics backend; a separate `PhysicsBody` role opts an eligible transform into
simulation.

## Authority model

Every transform degree of freedom has one writer in a step:

- **Pose-driven:** authored/input/animation/attachment pose is authoritative;
  velocity may be derived for observation. A backend receives the pose as a
  kinematic target if it participates in physics.
- **Velocity-driven:** the built-in integrator advances pose from explicit
  linear/angular velocity. This supports the initial scripted broom flight.
- **Backend-simulated:** forces, contacts, and constraints are stepped by the
  selected backend; its resulting pose and velocity are authoritative.

Authority changes—grab, release, mount, dismount, teleport, backend loss—are
transactions. They define whether pose is preserved, velocity is inherited or
zeroed, accumulated forces are cleared, and backend state is recreated. Two
integrators must never advance the same transform.

Attachments remain engine-level relationships. A backend adapter may represent
some attachments as joints or kinematic following, but it must report that
choice and cannot silently reverse follower/target ownership.

## Fixed-step simulation and frame synchronization

Physics uses a fixed simulation step independent of render frame rate. The host
accumulates elapsed time, runs a bounded number of substeps, and exposes
interpolated presentation transforms where needed. Define maximum accumulated
time and dropped-step diagnostics to prevent an overloaded frame from causing
an unbounded catch-up spiral.

Per frame:

1. Commit authored topology and component changes.
2. Resolve authority transitions and pose-driven targets.
3. Synchronize dirty shapes, body settings, and kinematic targets to the backend.
4. Apply queued impulses and per-substep forces.
5. Step zero or more fixed substeps.
6. Read back simulated poses and velocities in a deterministic component order.
7. Propagate transforms and publish contact/trigger observations.
8. Render using the final or interpolated presentation pose.

The precise engine schedule must reconcile cameras, XR tracking, grabbing,
AvatarControl, transform streams, and collision/zone queries. Consumers must
know whether they observe pre-step intent, authoritative simulation state, or
presentation interpolation.

## Backend interface

Define an internal trait around batched lifecycle and stepping, not one virtual
call per body per operation:

```rust,ignore
trait PhysicsBackend {
    fn apply_changes(&mut self, changes: PhysicsChangeBatch)
        -> Result<(), PhysicsBackendError>;
    fn step(&mut self, step: PhysicsStepInput)
        -> Result<PhysicsStepOutput, PhysicsBackendError>;
    fn query(&self, query: PhysicsQueryBatch)
        -> Result<PhysicsQueryOutput, PhysicsBackendError>;
    fn remove_world(&mut self);
}
```

The boundary uses stable engine IDs plus opaque backend handles. Backend types
must not leak into serialized MMS components, attachment rules, zone APIs, or
general engine events. Capabilities are queried explicitly: supported shapes,
joints, continuous collision, sensors, character control, determinism, and
threading may differ.

Provide:

- a null backend for scenes with no dynamics;
- a small built-in velocity integrator for velocity-driven pose motion;
- a mock backend for lifecycle, ordering, failure, and synchronization tests;
- later, one separately selected third-party adapter.

Backend failure disables or removes only backend-owned simulation state and
leaves valid component trees. Define whether the fallback freezes bodies,
converts them to pose-driven state, or stops the scene; never silently run a
second incompatible simulation.

## Performance model

Design for batches and dirty sets from the beginning:

- synchronize only created, removed, reparented, reshaped, or changed bodies;
- keep static geometry cached and update it only when its effective transform or
  shape changes;
- let sleeping backend bodies avoid transform writeback until woken;
- batch force/impulse submission and pose readback;
- avoid cloning the component world or rebuilding every body every frame;
- keep broadphase/narrow-phase work off render-critical locks where practical;
- bound cross-thread queues and expose backlog rather than silently increasing
  latency;
- allow multiple independent physics worlds/scopes without global scans.

Required counters include active/static/sleeping bodies, dirty synchronizations,
substeps, dropped accumulated time, broadphase candidates, contacts, sensors,
query batches, bytes or records crossing the backend boundary, and step/readback
time.

Profile representative workloads: one avatar in a room, the E2 broom scene,
hundreds of sleeping props, many moving props, and many zones with few active
attachment probes.

## Queries, zones, and events

Core synchronous point/shape queries remain available without a dynamics
backend. This keeps release-time attachment eligibility deterministic and makes
headless/script tests inexpensive.

A backend may accelerate ray, overlap, sweep, and sensor queries. Results are
normalized into engine IDs and engine-owned event types. Event publication must
define started/persisted/ended semantics, ordering, filtering, and behavior when
a participant is removed during delivery.

Do not require every zone to be registered as a simulated sensor. A few bare
zones can use direct core queries; zones with `Collidable` roles enter physical
collision; spring-exclusion zones remain solver-local unless deliberately
shared. Many continuously observed zones may opt into an accelerated backend or
shared broadphase.

## Delivery slices

1. Finalize `VelocityComponent` and `AngularVelocityComponent` storage,
   authority, scripting, serialization, and update timing.
2. Implement the built-in fixed-step velocity driver with linear and angular
   motion, no forces or contacts, and explicit authority conflicts.
3. Add force/impulse request semantics and a minimal force integrator suitable
   for tests, without presenting it as a rigid-body engine.
4. Define and test backend lifecycle/change batches with a mock backend.
5. Select and prototype one third-party adapter against the capability and
   performance matrix.
6. Integrate contacts/constraints deliberately; do not resurrect the retired
   collision-response solver as an implicit fallback.

The static non-penetration migration can proceed in parallel conceptually, but
only one component may correct/integrate a given movement target in production.

## Acceptance criteria

- Velocity and angular velocity have one explicit writer and defined spaces and
  units.
- Equivalent elapsed time produces equivalent velocity-driven movement across
  render frame rates within fixed-step tolerance.
- Force and impulse lifetime semantics cannot accidentally double-apply across
  substeps or frames.
- Grab/mount/dismount authority changes preserve valid pose and documented
  velocity without double integration.
- A mock backend can create, update, remove, step, query, and fail without
  leaking backend handles into authored state.
- Static geometry and sleeping bodies do no per-frame synchronization work when
  unchanged.
- Zone and attachment eligibility works with the null backend.
- Performance counters make queue latency, substep overload, synchronization,
  and collision workload visible.

## Related work

- [Gravity and acceleration velocity drivers: ownership comparison and next slice](gravity-and-acceleration-velocity-drivers.md)
- [Spatial, collision, and physics naming](spatial-collision-and-physics-naming.md)
- [Scriptable Velocity pose driver](scriptable-velocity-pose-driver.md)
- [Velocity / AngularVelocity components WIP](wip/velocity-components.md)
- [Retire collision response to static non-penetration](retire-collision-response-to-static-nonpenetration.md)
- [Interaction zones on the collision-query foundation](interaction-zone-collision-query-foundation.md)
- [Broom flight follow-up](broom-flight-followup.md)
