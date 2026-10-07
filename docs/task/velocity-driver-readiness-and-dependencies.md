# Task: velocity-driver readiness and dependencies

Status: AVC capsule-ready event and scene-authored gravity activation implemented,
2026-10-06. A general dependency API remains deferred.

## Problem and intended outcome

An avatar's movement root can exist before its runtime collision capsule.
In the Corp XR scenes, Gravity starts immediately, while AVC waits for a valid
InputXR pose, imported avatar geometry, and humanoid-map initialization before
creating its slide capsule. Without that capsule, neither static contact nor
the teleport reset sensor observes a mover. The avatar can fall below both.

The initial workaround in `VelocitySystem::avatar_contact_pending` traversed
the driven transform's subtree during each gravity substep, read AVC's private
capsule state, and called AVC's movement-target resolver. It withheld gravity
acceleration only; existing and commanded velocity still integrate. The user
confirmed startup was usable after that fix. The event-based replacement
preserves startup deferral while removing the Velocity-to-AVC coupling.

Model the actual dependency explicitly: a Velocity may be receptive to some
velocity drivers only when a condition is satisfied. Resolve that condition
outside physics substeps; substeps should consume cached eligibility.

## What exists today

- `Velocity.enabled` controls integration. It is not a separate driver-input
  gate. Runtime `translate` / `translate_world` add speed; `reset` clears it.
- Gravity is an ancestor provider, with nearest-provider selection and Velocity
  scope boundaries. Its authored `enabled` and `coefficient` settings exist.
  `Gravity.set_enabled(bool)` is now exposed as a runtime method. It changes
  enabled state without resetting existing velocity or the gravity coefficient.
- The public import event is `GLTFInitialized`, not `GLTFLoaded`. GLTF emits it
  after constructing the imported subtree and queuing startup pose application.
  It does not assert that AVC has completed splicing, obtained a valid XR pose,
  created its capsule, or published usable contact geometry.
- `ComponentRef` supports query and GUID references. Resolving a reference
  establishes component existence, not necessarily readiness. An authored GLTF,
  AVC, or Zone component can exist before the condition we need is satisfied.
- The generated AVC capsule is runtime state and excluded from serialization.
  Its identity should not become a fragile saved dependency GUID.

## Options

| Option | Contract | Advantages | Costs and limits |
| --- | --- | --- | --- |
| Scene enables Gravity after an event | Start the provider disabled; enable it when the game's chosen prerequisite is satisfied. | Small, explicit scene policy; Velocity stays independent of avatars. | Needs runtime Gravity activation and a suitable readiness event. `GLTFInitialized` alone is insufficient for XR capsule readiness. Each scene must handle removal, retries, and missed events. |
| Velocity has a driver-acceptance flag | A separate field controls contributions from velocity drivers while integration remains enabled. Scene code changes that flag. | Models receptiveness directly; reusable for loading, scripted control and motion authority. | A flag does not identify the dependency or maintain itself. Must define which inputs count as drivers and who may change the flag. |
| Velocity has a `ComponentRef` dependency | Resolve a prerequisite outside substeps and cache whether driver contributions are allowed. | Declarative, inspectable and reusable across avatar and non-avatar scenes; lifetime changes can invalidate it. | Existence alone only works when the dependency component is created exactly at readiness. Referencing AVC or GLTF requires an additional readiness contract. Query ambiguity and reference lifecycle need rules. |
| A conditional wrapper gates a provider | A readiness/dependency wrapper controls Gravity or another selected velocity driver. | Expresses the current gravity-only requirement directly; other drivers and explicit velocity remain usable. | Adds another authoring construct and scope semantics. Needs a cached dependency resolver and readiness producer just like a Velocity dependency. |
| Contact/AVC publishes readiness for a movement target | Runtime registration maintains a target-to-readiness association; scheduling supplies eligibility to gravity. | Removes repeated traversal and can cover existing scenes without editing them. | If implicit, eligibility remains hidden scene policy. Avoid moving the same AVC coupling into another general motion system or letting one avatar clear another's readiness state. |

These options can compose. A reusable dependency resolver can feed either a
Velocity acceptance gate or a provider gate; an event can also update an explicit
flag. ComponentRef resolution and a readiness event solve different parts of
the problem.

## Define what the gate controls before choosing its placement

Do not overload `Velocity.enabled` to mean driver readiness. Separate:

1. Integration: existing velocity advances the owned transform.
2. Continuous driver contributions: Gravity and future Acceleration change speed.
3. Discrete commands: `translate` / `translate_world` add speed.
4. Pose input: Input and InputXR can change transforms independently of Velocity.
5. Contact: constraints remove inward speed and correct penetration.

For the current startup fix, only gravity acceleration waits. Existing speed,
explicit speed commands and contact remain active. A future general
`accept_drivers` flag must not silently change that policy to block all inputs.
Raw tracked head/hand poses should not be held merely because gravity is waiting.

If the gate covers several continuous drivers, specify whether Gravity,
Acceleration, and future forces are all gated or whether each driver can have
its own dependency. If it also covers discrete commands, specify whether a
blocked command is rejected with a diagnostic or discarded. Do not accumulate
blocked commands or elapsed acceleration and replay them on readiness.
`reset()` must remain usable while gated, and contact must still be allowed to
remove unsafe speed. Closing a gate does not automatically erase existing speed.

## ComponentRef-based design sketch

The following names are illustrative proposals, not working MMS APIs:

```mms
let motion = Velocity
    .driver_dependency("[name='player_contact_ready']") {
    T { name = "player_root" /* avatar */ }
}
Gravity { motion }
```

This describes acceptance of continuous drivers, which is broader than the
current gravity-only workaround. A provider-scoped alternative is:

```mms
Gravity.requires("[name='player_contact_ready']") {
    motion
}
```

A dependency must identify something whose semantics match the condition:

- **Existence dependency:** a runtime marker is created only after the capsule's
  zone, movement-target routing, and contact frame are usable. Removal makes the
  dependency unsatisfied. This avoids teaching Velocity about AVC internals.
- **State dependency:** a stable readiness component exists from authoring and
  publishes pending/ready/failed state. ComponentRef resolves its identity;
  its state determines eligibility. More lifecycle information is available,
  but a generic consumer needs a defined readiness interface.

Referencing `GLTF` merely because it exists does not wait for import. Likewise,
referencing an authored `Collidable` does not prove that its zone/frame or target
has resolved. Prefer an explicit readiness producer over interpreting arbitrary
component types in Velocity.

AVC can publish contact readiness after initialization, while a different
producer can publish terrain, controller, or other game-specific readiness.
Scope markers to the specific player/movement target; a global first-match
`contact_ready` query is unsafe with multiple avatars.

## Cached resolution and lifecycle

Resolve dependencies during registration/configuration changes and runtime
readiness transitions, before the physics phase. Cache a boolean or compact
eligibility state for substeps. Neither query evaluation nor component-tree
traversal belongs in the fixed-substep path.

An unresolved dependency is pending and denies the gated contributions. It
must be able to resolve when its producer appears later. Invalidate the cache
when references, query matches, readiness state, components, or relevant scope
change. Define query ambiguity rather than choosing an arbitrary match.
Use the existing component-reference lifecycle facilities where suitable;
do not assume a successful lookup remains valid forever.

Publish ready only after the capsule is committed and its world frame and
movement target are usable by contact in the same scheduled step. On teardown,
reload, or retargeting, revoke readiness before the next affected substep.
With multiple prerequisites, require all authored dependencies; avoid a single
last-writer boolean when multiple producers can independently block readiness.
Cycles and failed dependencies should have diagnostics rather than silently
leaving an avatar permanently waiting. Runtime resolution caches and generated
marker identities must not be serialized as persistent authored state.

## Event-based alternative

A scene can own the startup policy by authoring disabled Gravity and enabling
it after an appropriate readiness event. This is attractive for a small game
and avoids a new general dependency API.

Use `GLTFInitialized` only when import completion is truly the scene's entire
prerequisite. For Corp XR avatars, introduce or expose contact readiness after
AVC creates the usable capsule instead. Define a queryable current state as
well as an event so late subscribers can initialize correctly. Removal or
reload needs a corresponding return to pending; a one-time enable callback
alone does not cover that lifecycle.

## Recommendation and decisions still open

### XR startup, pose access, and event composition

The removed workaround ran for any enabled Gravity contribution to Velocity,
even when the subtree contained no AVC or GLTF. Its traversal only blocked gravity
if it finds a collision-enabled AVC targeting that transform with no capsule.
It did not directly check GLTF components. That traversal has now been removed.

InputXR has internal `pose_valid` state, cleared each XR update and set after
a usable headset pose is applied. MMS currently exposes neither a pose-valid
getter nor a first-valid-pose event. It does expose the driven Transform's
`trs()` and `world.trs()` values. Those values can be authored defaults or the
last tracked pose, so transform access is not proof of tracking validity.

Startup has three distinct milestones: model import, a usable XR pose, and
AVC's resulting usable capsule. Import and the first pose can arrive in either
order. Combining just those two events can still enable gravity before AVC
has committed its capsule. A contact-ready state/event is the most direct
prerequisite for a grounded avatar. Other games may legitimately choose model
readiness, XR readiness, terrain readiness, or no dependency at all.

Avoid requiring nested callbacks: they can miss an event that happened before
the inner subscription was installed. A Promise.all-like startup latch needs
stateful readiness sources, current-state access for late subscribers, and
order-independent subscriptions. A persistent all-ready operator instead
recomputes on readiness loss and recovery; that is a different policy from
waiting once. There is no implemented Promise.all/RxJS readiness-combination
API in MMS today.

A first-pose event would be useful independently, but it should expose or pair
with queryable readiness and define reset on a new XR session. It is not a
substitute for capsule readiness. Distinguish a session's latched first usable
pose from frame-local tracking validity. Temporarily losing tracking after
startup should not silently acquire a policy of disabling gravity unless the
scene explicitly requests that behavior.

The first public scripting slice now provides a queryable contact-ready state
plus a transition event for grounded avatars. A small all-ready helper over
stateful sources can follow when scenes need more than one independent
prerequisite. Keep Gravity activation explicit and remove avatar discovery
from Velocity once the examples use the actual prerequisite.

Prefer explicit dependency/receptiveness semantics with cached readiness over
implicit avatar discovery in Velocity. For the present bug, a provider-scoped
gravity dependency preserves the narrow behavior most directly. A Velocity
driver-acceptance gate is useful if we intend several continuous drivers to
share one readiness condition. Do not select the broader gate just because
Gravity is currently implemented inside VelocitySystem.

Before implementation, choose gate placement, blocked-driver scope, and an
existence marker versus a state-bearing readiness component. Scene event
activation remains a viable smaller alternative if a general dependency API
has no additional concrete consumer yet. Keep these choices explicit rather
than introducing both public APIs in the first slice.

## Implementation and validation plan

1. Settle the acceptance contract and proposed authoring syntax.
2. Add readiness publication and cached dependency invalidation outside substeps,
   or add provider activation plus contact-ready event/current-state APIs for
   the scene-event option.
3. Migrate the five Corp scenes and `rei(mu).mms` where explicit authoring is
   required. Preserve desktop and XR initialization behavior.
4. Remove `avatar_contact_pending()` and make AVC's movement-target helper
   private again if no other consumer needs it.
5. Update the startup regression to exercise actual readiness transitions,
   including late XR pose, delayed GLTF import, removal/reload, missing and
   ambiguous references, retargeting, and multiple independent players.
6. Verify existing velocity integrates while gravity waits, blocked acceleration
   does not accumulate, reset/contact remain active, and readiness does not
   enable unrelated providers. Test acceptance of discrete commands according
   to the selected contract.
7. Verify no subtree traversal or dependency query runs per physics substep.
   Retest the real Corp scene with a headset after delayed startup.

## References

- [Avatar startup fall report](../bugs/mittens-corp-avatar-falls-before-capsule-ready.md)
- [Gravity and acceleration velocity drivers](gravity-and-acceleration-velocity-drivers.md)
- [AVC upright capsule](avc-upright-character-capsule.md)
- [Velocity and pluggable physics](velocity-forces-and-pluggable-physics.md)
- Runtime: `velocity_system.rs`, `avatar_control_system.rs`, `gltf_system.rs`,
  `component_method_registry.rs`, and `component_registry.rs`.

## Implemented slice: AVC capsule readiness

`Gravity.set_enabled(bool)` is implemented. Eight avatar MMS examples start
the player provider disabled and enable it from the owning AVC's `CapsuleReady`
data event: the five `mittens-corp*` variants, `rei(mu).mms`,
`secondary-motion-desktop.mms`, and `vtuber-secondary-motion.mms`.

```mms
Gravity.enabled(false) { name = "player_gravity" player_motion }
on(avatar, "DataEvent", fn(event) {
    if event == "CapsuleReady" {
        query("[name='player_gravity']").set_enabled(true)
    }
})
if avatar.capsule_ready() {
    query("[name='player_gravity']").set_enabled(true)
}
```

AVC owns readiness. It creates the capsule during normal initialization,
which in XR already requires a valid headset pose and initialized humanoid map.
Readiness is checked on a subsequent AVC tick, after initial transform
propagation. A usable enabled zone, enabled collidable, resolved movement target,
and contact-supported world capsule frame are required. The first ready
transition emits `DataEvent { name: "CapsuleReady", payload: Some(zone) }`
scoped to AVC; stable readiness does not emit every frame. MMS's existing
DataEvent callback receives the event name. `avc.capsule_ready()` reads the
published runtime readiness state and supports late registration.

The current state clears on an AVC readiness check if collision is disabled,
its contact components become unavailable, its target no longer resolves, or
its frame is unsupported. A later usable transition can emit again. Scene
activation is a startup latch: these examples do not automatically disable
gravity on readiness loss. General reactive loss/reload policies remain
follow-up work, not hidden Velocity behavior.

Model import callbacks remain responsible for animations/camera setup, but
no longer enable gravity. Velocity's avatar discovery, subtree traversal,
and dependency on AVC's movement-target resolver are removed. That resolver
is private to AVC again. Explicit velocity integration remains independent
of avatar readiness; disabled Gravity simply contributes no acceleration.

The named provider lookup occurs in the event callback, not per substep.
Captured provider handles also support the setter. Named queries assume one
player provider in each standalone example; multi-player games should use
player-scoped references instead.

Rust gravity examples use procedural bodies rather than delayed avatar import
and were not migrated. Examples without a Gravity provider need no activation
callback. Regression checks cover readiness waiting for XR initialization,
resolved target routing, deferred publication, once-per-transition emission,
readiness queries, GLTF import alone leaving gravity disabled, and the scoped
CapsuleReady callback enabling gravity in all eight MMS scenes.
