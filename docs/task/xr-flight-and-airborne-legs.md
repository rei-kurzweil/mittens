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

Flight suspends gravity on the Velocity driving the locomotion transform; it
does not disable collidable zones. The existing swept avatar contact path still
constrains movement against walls and floors. Disabling/removing the controller
restores gravity. Temporary loss of XR input clears tap history and leaves an
already-flying avatar hovering. Flight mode and tap history are runtime state,
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
