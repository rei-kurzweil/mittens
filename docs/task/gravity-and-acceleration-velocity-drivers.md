# Task: gravity and acceleration as velocity drivers

Status: ancestor topology and Velocity boundary rule selected; next slice planned,
2026-10-04. Gravity and Acceleration must be direct or indirect ancestors of
the Velocity they drive. Drivers do not apply across nested Velocity boundaries.
Gravity override rules and MMS names below remain proposals, not implemented
API. Start by migrating gravity
to first-class velocity; add a general acceleration provider using the same
ownership and stepping rules.

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
and currently combines elapsed fixed steps into one displacement. Its
`translate` and `translate_world` methods add a one-shot change of speed.
They do not describe persistent acceleration.

Today's `Gravity` supplies a coefficient to legacy `CollisionResponseSystem`.
At responder registration, the nearest enabled gravity ancestor wins. The
response component holds the resulting private velocity; `Gravity` does not
drive `VelocityComponent`. That lookup is not the proposed new contract.

The zone/contact migration now provides `Collidable.slide()` against static
zones, including desktop crates, bounded separation passes, and downward
capsule/floor crossing. It currently corrects poses only. Falling requires
updating velocity on contact and resolving contact within each fixed substep.
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
- Proposed gravity override policy: within that segment, the nearest Gravity supplies gravity. Other Gravity
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
