# Native rocket support module boundaries

Status: proposed. Velocity integration, component registration, and signal
conversion files are oversized. Keep acceleration and projectile sweep logic in
cohesive dedicated modules, with small integration calls in existing files.
Future extraction should separate motion-provider resolution and signal payload
conversion without per-step resource creation or per-draw allocations. Preserve
fixed-step integration and retained collision broad phases; validate motion,
serialization, and high-speed impact behavior with focused tests.

Move XR event name registration and payload conversion beside the XR gamepad
bindings when splitting the script dispatch files. Keep direct dispatch and
emit only locomotion state changes so thumbstick
samples never add a second per-frame scripting route. Exercise strict MMS
loading and synthetic XR takeoff, movement, release, and tracking loss.

If the avatar equipment tests keep growing, split the XR flight and thrust
fixture from the drop-zone tests. Share scene setup and fake grab helpers so
both suites exercise the existing collision path without extra render setup.
