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

Velocity now withholds gravity acceleration for a movement target with a descendant,
collision-enabled AVC targeting that transform while its capsule is pending.
Explicit velocity continues integrating during this wait. Once the capsule
exists, gravity acceleration resumes and contact can constrain the resulting pose.
Collision-disabled avatars and ordinary velocity-driven objects keep their
existing behavior. The guard applies to all similarly authored Corp derivatives.

The regression test holds a nested XR avatar at its spawn for ten simulated
seconds without a capsule, checks zero accumulated fall speed and continued
commanded velocity integration, then checks
normal gravity on capsule readiness and the collision-disabled bypass.
Live headset validation remains necessary to confirm the reported session's
failure was this startup gap rather than another contact or transform issue.
