# Task: gravity and acceleration as velocity drivers

Status: provider discovery, fixed-substep contact, Gravity runtime migration,
and legacy demo migration implemented and tested, 2026-10-04. Gravity must be
a direct or indirect ancestor of the Velocity it drives, with inheritance
stopping at Velocity boundaries and nearest Gravity winning even when disabled.
The desktop avatar grounding-root/AVC demo is implemented; XR scheduling remains pending;
general Acceleration follows those slices.

## What the existing docs and code say

The [driver terminology](../spec/physics/driver-terminology.md) and
[scriptable Velocity task](scriptable-velocity-pose-driver.md) distinguish a
**pose driver**, which changes a transform, from a **velocity driver**, which
changes velocity. Gravity belongs to the latter. The broader
[physics task](velocity-forces-and-pluggable-physics.md) describes persistent
gravity providers, transient forces, and explicit motion authority, but does
not settle gravity's component topology.

The implemented `Velocity { T { ... } }` owns linear speed in m/s and drives
exactly one immediate child transform. It stores velocity in the nearest
transform ancestor's orientation, compensates for parent scale when moving,
and integrates each fixed substep before static contact. Its
`translate` and `translate_world` methods add a one-shot change of speed.
They do not describe persistent acceleration.

`Gravity` now accelerates first-class `VelocityComponent` state each fixed step.
The old responder-registration coefficient cache and private gravity integration
have been removed from `CollisionResponseSystem`. The two legacy gravity demos
now use ancestor Gravity, Velocity-owned movement, and Zone/Collidable static
contact. Their old push response and cube-touch callbacks are retired.

The zone/contact migration now provides `Collidable.slide()` against static
zones, including desktop crates, bounded separation passes, and downward
capsule/floor crossing. It corrects poses and removes inward speed from the Velocity directly owning
the corrected transform, with contact resolved within each fixed substep.
The older [gravity/XR analysis](../analysis/velocity-gravity-xr-ground-contact.md)
describes that flow, although its collision and avatar inventory predates the
zone migration.

The [Velocity WIP](wip/velocity-components.md) and broader physics task contain
older proposals for world-space storage or velocity attached to transforms.
This slice builds on the implemented parent-local pose driver; changing that
storage contract is a separate decision.

## Ownership options

| Rule | Authoring and behavior | Advantages | Problems |
| --- | --- | --- | --- |
| Any gravity ancestor affects every descendant Velocity | `Gravity { ... Velocity { T { ... } } }` | Convenient scene-wide configuration | Unrelated motion layers become opted in. Nested velocities can accelerate the same descendant placement twice. Reparenting changes gravity eligibility. |
| All gravity ancestors contribute | Same topology, with summed acceleration | Supports deliberate stacked fields | Accidental nesting multiplies gravity. Needs explicit combination rules and a way to stop inheritance. |
| Nearest gravity ancestor wins | Search upward from each Velocity | Overrides work naturally; resembles legacy lookup | Still implicitly affects nested bodies and motion layers. Must define whether disabled gravity blocks inheritance or exposes the next ancestor. |
| Gravity must be Velocity's immediate parent | `Gravity { Velocity { T {} } }` | Local, visible opt-in; no arbitrary ancestor search | Couples a provider to wrapper placement. Adding another component between them changes ownership. Multiple different acceleration providers do not fit naturally in a single parent chain. |
| Gravity is Velocity's immediate child — rejected | `Velocity { Gravity {} T {} }` | Easy sibling composition | Violates the selected rule that a driver is an ancestor of the quantity it drives. |
| Only outermost Velocity nodes inherit ancestor gravity | In each tree branch, skip velocities with a Velocity ancestor | Prevents one common double-gravity case | Tree position is not body identity. A separately simulated passenger or held/released object can be nested under a vehicle. Inserting an unrelated outer motion layer silently stops its gravity. |
| Gravity names an explicit descendant Velocity target | `Gravity.target(vel) { ... vel ... }` | Selective targeting within a scope | Must validate ancestry as well as the reference. Targets outside the subtree are invalid under the selected topology. |

“Any ancestor” also needs a choice between nearest-wins and summation; those
are different behaviors. “Outermost” needs a definition of forest roots,
disabled Velocity ancestors, and how mounts or attachments change ownership.
Those rules are more complex than a topology test first suggests.

Nested velocities can represent separate channels or separate bodies. For
example, an outer falling placement and an inner locomotion layer should not
both receive gravity, but two independent bodies may legitimately be nested
for authoring. Do not globally disable nested Velocity integration to solve
gravity duplication. Decide gravity participation on the specific owner.

## Selected topology: velocity drivers wrap Velocity

A pose driver is an ancestor of the transform it drives. Use the same direction
for velocity drivers: Gravity and Acceleration are ancestors of the Velocity
they drive. Direct and indirect ancestry are both valid. Providers have no pose
of their own; intervening transforms supply the usual coordinate frames.

Provisional MMS:

```mms
let motion = Velocity {
    T { name = "grounding_root" /* avatar and collidable proxy */ }
}
let acceleration = Acceleration.vector([0.0, 0.0, 0.0]) {
    motion
}
let gravity = Gravity.coefficient(1.0) {
    acceleration
}
```

A transform or other ordinary component can occur between a provider and its
Velocity. Multiple providers compose by nesting wrappers, rather than by
placing provider children under Velocity. A provider may contain multiple
branches; providers reach the first Velocity on each branch and stop there.
Child or sibling providers do not drive that Velocity. A
provider never invents a missing Velocity component.

`ACC {}` is a candidate shortform for `Acceleration`, subject to checking the
shortform registry. Constructor and method spellings are proposals. Velocity's
existing requirement of exactly one immediate child Transform stays in place.

## Selected scope rule: stop inheritance at a Velocity boundary

Gravity and Acceleration do not apply across a nested Velocity boundary.
Provider discovery must obey these rules:

- For each Velocity, inspect its ancestor chain up to the next Velocity
  ancestor or the world root. Ordinary components and transforms do not stop
  the search. Provider discovery uses actual component-tree ancestry, not
  transform-frame references or arbitrary target lookups.
- Selected gravity override policy: within that segment, the nearest Gravity supplies gravity. Other Gravity
  ancestors in the segment are overridden; they do not sum. A disabled nearest
  Gravity blocks gravity from farther ancestors, so an explicit off wrapper
  can suppress inherited gravity.
- Enabled Acceleration ancestors within the segment add their vectors in a
  stable order. Validate finite input and finite summed results.
- A provider above an outer Velocity does not also accelerate an inner
  Velocity. The inner layer already inherits the outer transform displacement.
  It can opt into additional acceleration through a provider between the two
  Velocity nodes.
- An independently falling nested body can author its own Gravity wrapper
  below the outer Velocity. A shared ancestor does not silently opt it into
  another acceleration contribution.

For example:

```text
Gravity
  Acceleration
    T.parent_frame
      Velocity.outer              receives both providers
        T.outer_motion
          Velocity.inner          receives neither outer provider
            T.inner_motion
          Gravity
            Velocity.other_body   receives its own Gravity
              T.body_motion
```

This differs from a global outermost-only rule: nested velocities still
integrate, and explicit drivers below a Velocity boundary remain eligible.
It also differs from immediate-parent-only ownership: both providers above
`T.parent_frame` reach `Velocity.outer`. An ancestor provider can drive the
first Velocity on each branch of its subtree.

Unrestricted descendant inheritance is rejected: an outer driver must not
accelerate both an outer Velocity and a nested Velocity. Any future explicit
descendant selection must also respect Velocity boundaries; a reference must
not bypass this rule.

Disable a provider to stop its contribution; existing velocity persists.
Disabling Velocity pauses its acceleration updates and integration, with no
catch-up acceleration when reenabled. Even
a disabled Velocity remains a scope boundary; enabling it cannot silently
change an inner layer's provider set.

Reparenting providers or velocities changes eligibility at a step boundary.
Velocity state stays with its Velocity component. A provider with no eligible
descendant Velocity contributes nothing; misplaced child-provider authoring
should produce a diagnostic. Spatial gravity fields and shared configuration
references can be added later without changing the required ancestor direction.

## Gravity, acceleration, and script state

Gravity is a persistent world-space acceleration provider. Start with
`[0.0, -9.81, 0.0] * coefficient` m/s². Preserve finite positive, zero, and
negative coefficients if retaining the existing coefficient API. It never
writes a transform or stores a private falling velocity.

Acceleration supplies a persistent vector in m/s². Default to world space;
add explicit parent-local space with the generic component if needed. Convert
the world vector into the owner's parent orientation each substep, ignoring
scale. Gravity must remain downward in the world under a rotated parent.
Velocity's `.horizontal()` affects local movement commands; it must not
flatten gravity or all acceleration contributions.

Provisional live methods on retained provider references:

```mms
acceleration.set_vector([0.0, 2.0, 0.0])
acceleration.set_enabled(true)
acceleration.set_enabled(false)
gravity.set_coefficient(0.5)
gravity.set_enabled(false)
```

Setters replace persistent provider configuration once at the next step
boundary. They never accumulate an extra impulse, spawn another driver, or
multiply the supplied vector by `dt` at callback time. Getters expose vector,
enabled state, coefficient, and eventual space setting. Reject non-finite
values without corrupting previous valid state.

Disabling acceleration is different from stopping motion: velocity continues
until an explicit velocity change or contact changes it. A one-shot jump uses
a change of speed on Velocity, not a one-frame acceleration toggle. Persistent
provider settings may serialize; current velocity remains runtime state.
Define serialization of live provider edits consistently with existing
component settings, rather than treating setters as impulses.

## Stepping and contact must land together for a falling avatar

Use one coordinator to order the existing systems per fixed substep:

1. Apply queued state edits and motion-authority transitions.
2. Resolve enabled providers for each eligible Velocity and sum acceleration.
3. Update the owner's stored velocity: `v += a * dt`.
4. Integrate that velocity into its child transform and propagate the proposed
   pose so the capsule query reads the current substep.
5. Resolve static contact and correct the movement target. In world space,
   remove inward speed for each blocking normal:
   `v = v - min(dot(v, n), 0) * n`. Convert back to stored parent-local speed.
6. Commit the corrected pose and velocity before the next substep. Publish
   cameras and dependent queries from the final corrected frame pose.

Providers contribute requests; the coordinated velocity step owns the state
update. `StaticContactSystem` remains responsible for contact, with an explicit
path back to the Velocity that owns the corrected movement target. It must
not clear some unrelated nearest Velocity's speed. An AVC proxy currently
routes correction to its locomotion target; introducing an outer falling root
requires explicit routing to that root and its owning Velocity. Desktop and
XR authority handoffs need that mapping verified.

Applying gravity once per rendered frame, batching several integration steps,
then correcting only the final pose is insufficient. A landed capsule must
have its inward velocity removed before the next substep. Walking off an edge
removes support and resumes falling; no permanent upward acceleration provider
is needed to stand on a floor.

## Practical slices: player gravity and collision

The next implementation slice is coordinated stepping plus flat-floor landing
(items 1 and 2 together). Start with a headless falling-capsule fixture and one
desktop demonstration. Static crates can block the player before movable-crate
pushing is implemented. General Acceleration and spring-bone changes are not
prerequisites.

1. **Coordinate each fixed substep.** Replace Velocity's batched displacement
   with acceleration, integration, transform propagation, contact correction,
   and final propagation for every fixed step. Contact must identify the exact
   movement target and its owning Velocity, rather than a nearby motion layer.
2. **Make one player fall and land.** Enable ancestor Gravity on an outer player
   Velocity driving a grounding root. Route the avatar capsule's correction to
   that root. Remove inward speed on floor contact while retaining tangent
   speed. Verify falling from rest, thin-floor crossing, landing, stable resting
   speed, provider/Velocity disable and reenable, and walking off an edge. Check
   rotated/scaled parents and equal fixed-step results at different render rates.
3. **Block walls and static crates reliably.** Feed blocking normals back to the
   owning Velocity and add horizontal capsule sweeps; discrete separation can
   miss fast crossings. Verify wall sliding, crate sides, corners, thin obstacles,
   and ceiling contact. Blocking lateral motion must not cancel downward gravity
   or unrelated tangent motion. Existing static crate zones are the first fixture.
4. **Complete desktop/XR scheduling and ownership.** Keep locomotion and falling
   ownership explicit. Publish cameras and interaction queries after the final
   correction, including the XR gamepad path. Verify proxy routing and
   mounted/unmounted movement-authority handoffs. The desktop demonstration in
   item 2 does not substitute for XR scheduling or live headset verification.
5. **Add movable crate pushing separately.** Follow the
   [surface-contact task](surface-contact-and-coupled-motion.md) to solve player,
   crate, and crate/wall motion together. This is not required for player gravity
   against static crates. Explicit pulling follows the pushing constraint.

Migrate or retire legacy gravity/push demos as part of removing their old
runtime path; do not make removal depend on the later pulling feature. The
broader delivery checklist below still governs provider topology, serialization,
and the eventual general Acceleration API.

## Delivery and acceptance

1. Implement provider discovery through direct or indirect ancestors, stopping
   at the next Velocity ancestor even when it is disabled. Finalize gravity
   override rules within that scope.
2. Make Gravity drive eligible descendant Velocity state and implement per-substep
   acceleration/integration. Audit legacy Gravity scenes; migrate or retire
   them before changing the existing ancestor API's runtime meaning.
3. Add contact normals and explicit velocity ownership to the falling-avatar
   slice. Verify landing, resting speed, leaving support, and tangential
   motion on one flat floor. Resolve XR camera publication ordering.
4. Add Acceleration and its MMS/live API using the same provider machinery.
   Finalize `ACC` only after checking naming and serialization.

Required checks cover nested independent bodies versus nested motion layers,
nested gravity overrides, provider and Velocity disable/reenable, removal
and reparenting, rotated/scaled parents, zero initial velocity beginning to
fall, and equivalent fixed-step results at different render rates. Contact
checks include thin-floor crossing, zero inward speed at rest, a ceiling,
free tangent speed, and support removal. Existing gravity-free Velocity scenes
must retain their behavior.

## Related work

- [Static non-penetration migration](retire-collision-response-to-static-nonpenetration.md)
- [Velocity, forces, and pluggable physics](velocity-forces-and-pluggable-physics.md)
- [Scriptable Velocity pose driver](scriptable-velocity-pose-driver.md)
- [Velocity, gravity, and XR ground contact analysis](../analysis/velocity-gravity-xr-ground-contact.md)

## Progress: provider discovery (2026-10-04)

`VelocitySystem::gravity_provider` reads actual ancestry, crosses ordinary
components and transforms, and stops at any Velocity ancestor. It selects the
nearest Gravity regardless of enabled state. Tests cover branched ownership,
independent nested bodies, disabled boundaries, off overrides, child providers,
reparenting, and provider removal. Runtime integration now uses this discovery
as described in the migration progress below.

Next: validate the desktop demo interactively, and verify XR publication and
movement authority.
Acceleration discovery and its API remain pending.

## Follow-up: surface contact and coupled motion

See [surface contact and coupled motion](surface-contact-and-coupled-motion.md)
for temporary pushing and explicit pulling relationships between movable
surfaces. That work extends contact beyond static non-penetration without
reviving legacy push response. Spring-bone gravity remains outside this scope.

## Progress: fixed-substep contact foundation (2026-10-04)

Runtime scheduling now advances Velocity one 120 Hz step at a time, flushes
transform propagation, resolves static contact, and propagates corrected poses
before advancing again. Contact runs before the first substep to seed capsule
sweep history and resolve input movement even when no fixed step is due.
The accumulator limit and dropped-time policy are unchanged.

Static contact projects inward world-space speed out of the Velocity directly
owning the corrected transform, then converts back to parent-local storage.
It does not search for a nearby ancestor Velocity. Existing explicit movement
target routing therefore selects both pose and velocity ownership.

Headless tests cover a fast downward crossing of a thin floor, zero inward
speed after landing, retained tangent speed, equal results at 60 and 120 Hz
render rates, and rejection of unrelated ancestor ownership. The six static
contact tests and existing Velocity tests pass.

This is the stepping/contact foundation for practical slices 1 and 2; those
slices remain incomplete. The subsequent migration below enables Gravity.
Remaining work includes live demo validation, rotated/scaled
parent contact checks, and XR publication/authority scheduling. General
Acceleration and horizontal sweeps remain later slices.

## Progress: Gravity runtime and demo migration (2026-10-04)

Gravity discovery now runs in each fixed Velocity step, applying world-space
`[0, -9.81, 0] * coefficient` in the owner's parent orientation without scale.
This starts stationary Velocity components falling and bypasses `.horizontal()`
command flattening. Disabled providers retain existing speed; disabled Velocity
pauses acceleration and integration. Finite coefficient setters reject invalid
values without changing the previous setting, and integration rejects non-finite
acceleration results before changing state.

`examples/gravity-fields.rs` and `examples/collision-perimeter.rs` now author
Gravity -> Velocity -> Transform for falling cubes. Cameras, floors, and walls
use Zone/Collidable contact. Zone shapes are in local space, so scaled geometry
uses unit-cube extents rather than duplicating scale in the shape. The gravity
coefficient cache and gravity path in CollisionResponse are removed. Legacy
pushable behavior and cube-touch callbacks are retired; coupled pushing remains
a separate task.

Tests now also cover falling from rest and sustained resting speed, render-rate
equivalence under gravity, world-down gravity under rotated/scaled parents,
horizontal command mode, live coefficient replacement, reversed gravity,
provider/Velocity disable and reenable, independent nested motion layers, and a
scaled demo-style cube landing and falling again when support is removed.
The demos compile with `cargo check --examples`. Desktop visual and live XR
validation remain pending alongside the avatar grounding-root integration.

Validation of this migration: 19 focused Velocity/static-contact tests pass;
`cargo check --examples`, `cargo fmt --check`, and `git diff --check` pass.
The full library suite reports 933 passed, 50 failed, and one ignored. An
unchanged HEAD snapshot reports 928 passed, the identical 50 failing test names,
and one ignored; no new full-suite failures were introduced in this run.

## Progress: secondary-motion player demos and jump (2026-10-04)

`secondary-motion-desktop.mms` now starts its avatar one metre above the floor
under Gravity -> Velocity -> grounding Transform. Desktop Input retains the
inner head transform for movement and look. `AvatarControl.movement_target`
explicitly routes the generated capsule's correction to the falling root;
unresolved explicit targets skip correction rather than falling back to the
head driver. This authored setting serializes with the AVC configuration.

`Velocity.grounded()` exposes upward blocking contact from the latest fixed
substep. Support state resets on an enabled Velocity step and an upward speed
command clears it immediately. It is runtime state, not serialized. The demo
uses grounded support rather than zero vertical speed to gate a 4.5 m/s jump,
on Space KeyDown or the camera-attached jump button. Keyboard repeat uses
KeyPress, so holding Space does not add a jump every frame. Existing Input
movement/look stays enabled. R/F remain existing direct vertical controls;
per-key overrides are follow-up work, not part of this slice.

The XR `vtuber-secondary-motion.mms` demo also wraps its locomotion root in
Gravity/Velocity, adds a static floor zone, routes AVC contact to that root,
and binds ButtonY through existing InputXRGamepad XrButtonDown events. The
button handler changes falling speed once while supported. Controllers without
Y need a separately authored alternative. This verifies authored ownership and
injected button dispatch; live XR scheduling/camera publication is still pending.

Headless desktop checks use the actual loaded Bisket avatar and generated
capsule, verifying falling, landing, support, retained inner pose, Space jump,
UI jump, rejection of airborne jump, and landing again. XR checks inject support
and button events without requiring a headset. The input-source/action-binding
terminology and default per-binding overrides are tracked separately in
[input actions and per-binding overrides](input-actions-and-per-binding-overrides.md).

Validation of the player-demo slice: the four desktop example tests, the XR
button fixture, 26 AVC/serialization tests, and 19 Velocity/static-contact tests
pass. Both Rust example launchers compile. A broader secondary-motion filter
still hits the existing desktop spring-chain count mismatch (15 versus 17),
also present in the unchanged baseline; it is outside player-gravity scope.
Interactive desktop and live headset validation remain outstanding.

## Demo follow-up: XR platform and editor footprint (2026-10-04)

Live testing of `load examples/vtuber-secondary-motion.mms` showed falling from
an off-platform spawn and slow rendering. The platform is now 40 x 40 metres,
centered on the tracking origin rather than offset in Z. Its top remains at
world y=-0.79. An authored EditorUI selects only Settings, with spring-bone and
zone controls; the other default workspace panels are omitted. Performance
improvement and actual tracked spawn coverage require another live check.

After validating those changes, consider a lower reset Zone that detects a
falling player and teleports the grounding root back onto the platform. Cone or
spike patches below that zone can make the reset boundary visible. Define the
safe destination, clear falling speed, and reset contact/sweep history together
so teleportation cannot be interpreted as a swept floor crossing. This reset
behavior and its visuals are deferred until the larger platform and reduced
editor workspace have been tested.

## Progress: legacy response removal (2026-10-04)

The remaining Bisket desktop camera and shared voxel terrain now use
Zone/Collidable contact. CollisionResponseComponent/System and their private
velocity, push behavior, scheduling, lifecycle intents, MMS API, and `CRSP`
shortform are removed. This completes response retirement; legacy Collision
intersection/event infrastructure remains for a separate audit and cleanup.

### Corp stages and fall recovery

Corp variants and the standalone Reimu stage scene now share stage contact
prefabs, Gravity/Velocity player motion, grounded Space/ButtonY jump bindings,
and a reusable teleport pit. Opt-in Zone observation, ordinary pose updates and `Velocity.reset()`
for respawning are described in
[zone-enter-events-and-teleport-pits.md](zone-enter-events-and-teleport-pits.md).
Mounted jump capability routing is deferred in
[mounted-action-capabilities-and-jump-routing.md](mounted-action-capabilities-and-jump-routing.md).

The transform teleport API has been removed. `Velocity.reset()` clears speed,
grounded state, and sweep history independently of pose updates. The pit queries
its mover's owning Velocity and uses normal local `update_transform` for respawn.
Queried and captured handles (including `let velocity`) are supported.
