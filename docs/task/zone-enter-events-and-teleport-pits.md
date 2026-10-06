# Zone enter events and teleport pits

Status: initial implementation; mounted action routing remains separate.

Previously, Zone was passive. Rust consumers could call synchronous overlap,
point and contact queries, but MMS had no Zone enter/exit signals. Legacy
Collision events belong to the older Collision detector and are not Zone events.

## MMS contract

The initial API was named `watch_slides()`. It is now `enable_events()`; event
observation is opt-in, while candidate filtering remains the first slice's
separate constraint. No events are emitted by a bare Zone.

`Zone.cube([...]).enable_events()` (also sphere/capsule) opts into observation of
enabled `Collidable.slide()` zones. A bare Zone remains passive. Observation
runs after frame pose updates; a new overlap emits `ZoneEntered`, and a live
pair separating emits `ZoneExited`, scoped to the observing Zone. Staying inside
emits neither. Static collidables are excluded, as are mounted movement roots.
Disabled or removed participants retire pairs without callbacks on stale handles.
Tangency counts as overlap. Query failures are skipped.

Payload fields are `zone`, `other_zone`, `collidable`, `movement_target`, and
`movement_target_offset`. The offset is movement-root local position minus the mover
zone center converted into the target parent coordinates, captured when observed. Component handles preserve their
catalog types in the callback transport, allowing normal MMS method calls.

Observation samples current poses and additionally catches downward capsule
crossings of upright box tops, including yaw. This protects horizontal teleport
pits from fast falling players. Arbitrary swept trigger shapes, upward crossings,
and moving sensors are not a general continuous collision detector.

```mms
import { teleport_pit } from "../assets/components/teleport_pit.mms"
teleport_pit("pit", [0.0, -14.0, 0.0], [100.0, 12.0, 100.0],
    [-5.0, 1.2, 0.0], "spikes")
```

The factory returns a Transform containing a non-solid observer. Position and
size describe its local volume; destination is the mover zone center in the
movement target parent coordinates. These equal world coordinates for the demo
rigs, whose outer parents have identity transforms. Keep destination outside the trigger. Decoration can be `"none"`,
`"spikes"`, or a zero-argument factory producing a fresh mesh component per patch.
Patches use deterministic jitter, so scene reloads preserve the layout. They do
not participate in physics. Cones sit below the trigger volume.

The callback queries the directly owning Velocity with
`event.movement_target.query("../Velocity")`, calls `velocity.reset()`, and uses
`event.movement_target.update_transform(destination + offset)` for the local
pose update. Position-only updates preserve rotation and scale. Normal pose
transition behavior is retained; respawn rigs should have immediate pose updates.
The removed transform teleport API combined motion reset and coordinate space;
Velocity now owns the motion reset independently.

`Velocity.reset()` clears linear speed, grounded state, and that driven target's
static-contact and observer sweep history. It does not change its pose, enabled
state, rotation basis or gravity configuration. Subsequent gravity steps resume
normally. A queried or captured Velocity handle can be reset directly. Clearing
observer history prevents a discontinuous reposition from sweeping through
intervening sensors.

## Demo integration

Mittens Corp, desktop, AGC, AGC desktop, linear velocity and `rei(mu)` now use
shared stage geometry with deck/step/wall contact zones. Studio floors and
suspended walkways also have static contact zones. Players explicitly route their
AVC capsule response to the outer Velocity movement root and receive Gravity.
Desktop Space and XR ButtonY jump when grounded. Corp pits return capsule centers
to the stage. The Reimu derivative uses an invisible pit below its lower stage.

Attachment transactions suspend the movement root's directly owning Velocity,
clear stale speed, and restore its prior enabled state on dismount. Mounted roots
are excluded from independent slide response and pit observation. Current car
bindings are unchanged; capability-based mounted/player jump fallback is tracked
in [mounted-action-capabilities-and-jump-routing.md](mounted-action-capabilities-and-jump-routing.md).

Uniformly scaled upright mover capsules support yaw. Upright yawed static boxes support capsule/sphere response and downward capsule
floor sweeps. Tilted/sheared static boxes and general moving-shape oriented
contact remain unsupported; do not approximate their response by world AABBs.

The secondary-motion desktop demo also includes its own pit below the 18 m floor,
covering a 120 m square. Its respawn center is `[0.0, 1.2, 1.0]`. Regression tests
move the inner Input driver beyond the floor and let gravity produce the entire
fall, then verify respawn, zero velocity, and landing again.

## Follow-ups

- Explicit role/candidate filters and spatial indexing: [optimization tracker](zone-observer-candidate-filtering-and-broad-phase.md).
- General swept triggers and sensor motion, if a concrete interaction needs them.
- Configurable respawn orientation and explicit retained momentum policy.
- Diagnostics for unresolved movement targets and unsupported observation queries.

## Validation

Headless integration covers both Corp and secondary-motion desktop avatar landing, Space jump, pit respawn, cleared
fall speed, and landing again. Synthetic tests cover fast downward crossings,
Input offsets, enter/exit deduplication, opt-in observation, disabled sensors,
yawed stage contact, and mount/dismount velocity ownership. All Corp variants
load, all targets compile, and the MMS crate tests pass. Queried and captured
Velocity reset tests also verify retained pose and configuration.

The last full library run passed 946 tests, with 50 failures and one ignored test.
The failed test names exactly match the unchanged HEAD baseline; this slice adds
no regressions. Interactive XR behavior still needs verification with a headset.
