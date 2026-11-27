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

Keep particle motion and retained instances in the shared particle system.
Keep rocket profiles and flame presentation in the rocket prefab. If the
frame coordinator grows, extract post-pose effects into a
single phase after XR and IK transforms settle. Keep direct instance updates,
share meshes and material identities across emitters, and rebuild draw streams
only when instances or their render phases change. When splitting the intent
and MMS registries, keep particle emission and lifecycle dispatch together.
Validate continuous hover emission, thrust transitions, removal, and stable
draw-cache topology while particles age. Profile render cost on a headset.

Keep connected trail pose and shared tube geometry in a native trail module.
Keep the rocket appearance and input transitions with the prefab. Extract
the trail and particle setters into the same effects binding family when
splitting the dispatch registry. Keep one retained mesh and a bounded native
pose history, and verify that tube deformation, fading and removal do not
rebuild meshes or require MMS frame callbacks.
Ship flow phase through the retained instance payload. Keep the shared shader
and material bindings stable across hover and thrust and test phase continuity
at both the speed change and the wrap point.
