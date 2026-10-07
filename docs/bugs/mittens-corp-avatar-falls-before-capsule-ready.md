# Corp avatar can fall below the studio during XR startup

Status: startup integration guard implemented 2026-10-06; headset verification pending.

Reported in `examples/mittens-corp.mms`: only the avatar and gray background
were visible, followed by increasing vertical instability and loss of vertical
vertex precision. This is consistent with a prolonged fall to large negative Y;
the original session's coordinates were not captured.

The scene's outer Velocity receives Gravity immediately. AVC initialization
waits for a valid ancestor InputXR pose and the imported humanoid map before
generating its slide capsule. Without that capsule, neither static contact nor
the teleport pit's zone observation sees a mover. The root can therefore fall
below both the studio and the finite reset sensor before initialization ends.
Creating a capsule below those volumes does not recover the missed crossing.

The final fix makes the scene's Gravity provider start disabled. AVC publishes
a scoped `CapsuleReady` data event after its generated zone, target routing and
contact frame become usable, on a later tick than capsule creation. The scene
then calls `Gravity.set_enabled(true)`. Model-import callbacks no longer enable
gravity, and Velocity no longer discovers avatars or traverses their subtrees.
AVC also exposes `capsule_ready()` for subscriptions registered after readiness.

Regression checks cover late XR readiness, missing targets, delayed capsule
readiness publication, stable once-per-transition emission, scene callbacks,
and desktop landing/jumping/reset behavior. The user confirmed the initial
startup fix made the scene usable; the event-based replacement still needs
live headset verification.
