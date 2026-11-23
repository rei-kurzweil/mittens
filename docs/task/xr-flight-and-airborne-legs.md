# XR flight and unsupported lower-body IK

`InputXRGamepad.enable_flying(true)` opts a controller into flight. The default
is false, preserving existing XR input behavior. Sakura's XR rig enables it:

```mms
InputXRGamepad { locomotion() speed(1.4) enable_flying(true) }
```

The right B button is jump. One press jumps at 4.5 m/s when the controlling
Velocity is grounded. Two distinct down edges less than 250 ms apart toggle
flight; holding the button is not a second tap. The second tap clears ballistic
velocity. During flight, hold B to rise and A to descend. The movement stick
follows the rendered headset orientation, including pitch. Flight speed is
exactly twice configured walking speed; combined stick/vertical input is bounded
to that speed. Another double tap leaves flight and restores gravity.

Flight owns a body-local `Gravity` override on the Velocity driving the
locomotion transform. It sets this instance to Off while flying and Auto while
walking, inheriting the current authored provider again. Disabling/removing the
controller removes only its own override. See [Gravity modes](gravity-modes.md).
Velocity carries no flight-specific gravity flag. Collidable zones stay active. The existing swept avatar contact path still
constrains movement against walls and floors. Disabling/removing the controller
restores gravity. Temporary loss of XR input clears tap history and leaves an
already-flying avatar hovering. Flight mode, tap history and the temporary Gravity override are runtime state,
not saved scene configuration. Mounting/input authority rules remain unchanged.

The existing lower-body system remains the sole leg writer. Without support,
WalkingIK releases planted contacts. Its airborne policy estimates horizontal
world velocity from capsule movement and moves both sole targets opposite that
velocity, with smoothed onset/reversal, bounded trailing distance and slight
knee flexion. It responds to sideways motion and body yaw, not just model Z.
An explicit flight mode forces airborne goals even while touching a floor.
Landing resets the trail and resumes supported foot placement. Tracking loss,
teleports and invalid frame times reset motion history. Walking opt-out and
mounted behavior retain their existing pose ownership. No upper-body solver is
introduced here.

Tests cover tap edges/window, opt-in/defaults, jump, double-speed movement,
vertical controls, gravity restoration, wall collision, MMS round-trip, sideways
leg trailing, direction reversal, flight over a floor and landing. These are
host regressions; headset appearance and controller feel still need device QA.

## Diagnosing a peer that keeps crawling in flight

Sakura Twilight sends root and head poses on separate channels. Before flight
replication, the remote PoseInput supplied neither flight state nor ground
support. The local leg solver could see the XR gamepad's flight flag and
select airborne goals. The torso's auto mode still used head height relative
to its last floor. In kneeling mode a
leg can report no support collider while returning a goal height carried
forward from the last floor. Near or below that old floor, losing contact
can leave the observer solving hands-and-knees against it in midair. Rising
far enough above the old floor should still release horizontal mode.

An offline Twilight Rooftop replay drove the remote Lumae through standing
and crawling, then moved its received root 15 m sideways and 2 m below the
roof with a standing-height head target. After one second it was still
horizontal, with `NoSupport`, both feet targeting the old 16 m floor, and
both knees 37 cm above the hips. Moving the root to 2 m above the roof
returned it to vertical. This reproduces the reported shape with synthetic
poses and gives a check for the next live recording.

Mitten folding when the wearer deliberately lies on their stomach is a
separate test. Horizontal mode currently produces hands-and-knees, with
knee and shin goals under the torso. It has no prone pose to transition to.

### Opt-in capture

Run each peer with `CAT_DEBUG_BODY_SOLVER=1` and keep the labeled local and
remote `[body]` lines from the same time window. This already samples every
half second. Record a low stance, flight toggle, hover, climb, and descent.
If the existing log is insufficient, extend this same opt-in path to record
flight edges at the source, root/head packet age and Y at the observer,
standing head height, and each foot's collider and goal height. Record
posture transitions with their reason through the existing PostureChanged
event. Read retained state only, with no extra raycasts or telemetry packets.

Compare the near-floor and descent cases with a climb that crosses the
normal exit threshold. If the observer follows the climb yet stays horizontal,
check packet age, received flight state, and floor updates. Confirm landing
reacquires support and a stationary crawl keeps its posture.

If the scene test module grows, move replay and network-pose diagnostics into
a separate fixture. Keep the normal body/leg path free of capture allocation
and limit support and transport diagnostics to opt-in capture.

## Replicating flight mode

Sakura Twilight publishes the current 0/1 flight state on join, after a
transition, and once per second for recovery after packet loss. The peer
feeds it to its existing PoseInput so avatar replacement
preserves it. Flight forces automatic torso posture vertical and selects the
existing airborne leg goals. Landing lets support reacquire normally. Validate
with the same below-roof replay and then with two live clients.

When maintaining the larger MMS registry and rooftop player, move pose-source
methods into the input binding family and separate player construction from
avatar switching. Preserve direct host dispatch and keep the solver's
per-avatar path allocation-free. Exercise MMS method dispatch and the replay
before and after either move.

## Flight permission from MMS

Use `gamepad.set_flight_enabled(bool)` on the local `InputXRGamepad`.
Permission enables the existing double tap. Revoking it clears flight and
restores the controller's gravity immediately. Authored jump behavior and
normal stick input continue while permission is withheld. The override
belongs to the live controller and is omitted from saved configuration.
`flight_enabled()` reads the permission and `flying()` reads active
flight. Peers still receive active flight through the existing channel.

Sakura Twilight Simpler grants permission when one loose STA rocket is
attached to each thigh slot. React to equipment topology changes, keeping
ordinary gamepad and IK paths free of per-frame equipment searches. Test
firmly attached rockets, a same-side or held rocket, takeoff and gravity
restoration after removing a rocket.

When separating the larger ECS intent and MMS dispatch files, move XR input
intents and host methods together beside the XR gamepad bindings. Preserve
direct dispatch for unrelated intents and verify both script and native
flight lifecycle tests. Keep scene-authored equipment policy separate from
shared avatar construction.
