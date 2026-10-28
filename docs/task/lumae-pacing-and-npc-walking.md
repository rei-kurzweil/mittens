# Lumae pacing controller and NPC walking eligibility

Date: 2026-10-28

Status: reusable MMS motion controller implemented and tested. Untracked NPC walking remains a
separate native eligibility/calibration gap; Sakura's plain GLTF Lumae have not been changed.

## Existing behavior

Walking defaults on in AvatarControlComponent, not on GLTF or the Lumae model. The gait measures
actual capsule-center world X/Z displacement after movement and contact resolution. It also handles
heading changes. It does not require a named locomotion input action or a start-walking signal.

Current LowerBodyIkSystem eligibility nevertheless requires an ancestor InputXR with an enabled,
valid pose, a resolved AVC/model/leg binding, explicit sole calibration, an enabled ready AVC
capsule, and supporting static geometry. A plain GLTF does not register a lower-body instance.
Sakura's spawned Lumae are currently placement transforms containing GLTF, relaxed arm pose,
colliders and secondary motion; they have no AVC. Consequently they do not get walking IK today.
A non-XR AVC can initialize its head/body splices, but that alone does not pass the lower-body
tracking gate. Do not fake tracking validity to disguise this gap.

## Implemented controller

Module: `assets/components/characters/lumae/behavior/pacing_and_waiting_around.mms`.

```mms
import { pacing_and_waiting_around } from "../assets/components/characters/lumae/behavior/pacing_and_waiting_around.mms"

let avatar_root = T.position(0.0, floor_y, 0.0) {
    // Model or already configured avatar hierarchy goes here.
}
let pacing = pacing_and_waiting_around(avatar_root, {
    speed = 0.35
    wait_seconds = 2.0
    turn_seconds = 1.2
    walk_seconds = 3.0
    turn_radians = 3.141592653589793
    ease_seconds = 0.35
})
pacing
pacing.start()
// pacing.stop()
// let snapshot = pacing.status()
```

Passing false instead of the options table selects the defaults above. Construction creates one
Velocity wrapper with avatar_root as its immediate transform child. Emit/attach that returned
wrapper instead of separately attaching avatar_root. Construction starts paused; start() enables
the controller and is idempotent. stop() stops planar/yaw motion and restarts the schedule at its
initial wait for a later start. status() returns a snapshot of running, phase and elapsed.

The repeating cycle is wait, eased turn, and eased forward travel. Alternating signed half-turns
make the default pace back and forth. Custom angles can create an unbounded route; this is timed
motion, not navigation or an edge-avoidance planner. Obstacle responses come from the existing
contact setup, and blocked motion does not make the controller accumulate scripted positions.

The controller reads the root's live local quaternion for its +Z forward. It adjusts actual linear
velocity rather than accumulating impulses: Velocity.translate() adds a delta, so each update
subtracts the live planar velocity before applying the desired forward value. Angular velocity is
similarly adjusted for an eased yaw turn. It never teleports the root, writes bones, or clears
vertical gravity/contact velocity. Its dedicated wrapper owns planar motion and angular velocity;
do not mutate its rotation_basis or combine another steering controller on that wrapper.

The root/parent are upright yaw frames. Speed is measured in parent-frame units/second, so scaling
the parent scales world travel. The avatar root's own scale is preserved. FrameTick script callbacks
commit commands after native update, so velocity is consumed at the next normal physics update.
Frames longer than the eight-substep Velocity budget (1/15 second) stop the commanded motion and
freeze the behavior schedule for that frame rather than advancing the schedule through lost time.

The existing on_global callback is scoped to the MMS session. There is no per-controller
unsubscribe/liveness contract in this module: call stop() before removing the controller/avatar or
closing its session while the remaining world continues to simulate.

## Validation

`cargo test --lib lumae_pacing` passes four integration tests through retained MMS callbacks and
the native fixed-step Velocity integrator:

- construction remains stationary until start, then wait/turn/walk produces real bounded motion
  under a rotated parent, with no velocity accumulation and preserved scale;
- stop preserves the root pose and existing vertical velocity;
- half-turn easing and final orientation agree at 30, 60 and 120 Hz;
- repeated start is idempotent and a long frame does not fast-forward the schedule.

No new engine, component, signal, or MMS host API was introduced. The module's start/stop/status
members are ordinary MMS functions on its returned controller, as with existing script prefabs.

## Next decision: native untracked NPC lower body

Supporting actual Lumae walking needs a truthful untracked rig entry point or an explicitly supported
non-tracked AVC path. That work should preserve tracking-loss behavior for XR, avoid tracked-head
splices imposing a player camera pose on NPCs, calibrate Lumae soles against its own rest skeleton,
and let the existing lower-body owner consume the resulting supported root motion. It should be
verified on the actual Lumae rig before enabling pacing in Sakura.
