# Body posture and support debug overlay

Status: implemented, headset validation pending. Enable **Panels → Editor →
Body support** in Sakura Twilight Simpler. It follows the selected avatar,
or the active avatar camera if the selection does not identify a rig. The
adjacent **Walking gait** toggle uses the same selection.

Enable **Paw capture HUD** in the same panel to show a paw button when the
selected or active avatar has a posture sample. Clicking saves the displayed
decision with its time, floor, contacts, and motion, and places a colored paw
at the avatar's location. Captures stay in the live scene when the HUD is
turned off.

The third-person camera worked in the playtest. The avatar sometimes stayed
in a plausible crawl pose at the wrong height above the surface. Getting in
and out of a vehicle appeared to clear it. Tiny-player playtesting is still
pending in the [player grabbing and equipment tracker](player-grabbing-and-tiny-avatar-equipment.md).

## Retained floor hypothesis

`BodySolverSystem` uses the lowest live leg contact or a body floor hit.
When both are unavailable it retains the last floor. Auto posture compares
head height to that floor, and entering a vehicle clears it. A probe centered
on the retained height can miss a new floor after a drop and keep the rig
horizontal. Capture the floor and source before and after a stuck transition.

Instrument the policy before changing it. A short loss of contact, a genuine
fall, and a stale floor after moving to another surface can currently look
similar in the avatar's pose.

Miku's scene-owned capsule can land while posture still remembers the upper
floor. For a kneeling rig without an avatar-owned collider, use its authored
locomotion origin to look for nearby support. Keep the remembered floor
during the drop and accept a new height only from a live surface. The green
and blue thresholds both reference that same floor.

## Overlay for one avatar

Make it easy to select the local avatar, a peer, or NPC Mitten in Sakura
Twilight Simpler without illuminating every rig. Retain one overlay and
clear it when disabled or when no eligible rig is available. A scene-authored
opt-in alongside `ik_debug()` can follow if needed.

In Sakura, use independent Body support and Walking gait toggles in the
existing Settings-only Editor dock item. Use the current scene selection and
keep `Editor.active()` empty. Verify that selecting a rig, using the cursor,
and toggling an overlay work with only Settings loaded and without building
the World or Inspector models.

- **Magenta support lines.** Show which points the current pose tries to
  support and their vertical distances to the surfaces actually sampled.
  Use soles for the vertical policy and knees for hands-and-knees. Mark each
  leg as planted, swinging, or unsupported. Label a remembered floor and
  render it distinctly from a live hit. Keep a remembered floor visible if
  it still drives posture after both live contacts are gone. Show the ray's
  sampled point separately when it differs from the solved contact. Add hand
  markers if a future solver uses hand support.
- **Green standing threshold.** From the tracked head reference, show the
  signed vertical distance to the exit from horizontal posture. The endpoint
  is the reference floor plus 60% of scaled standing head height.
- **Blue lowering threshold.** Show the signed vertical distance to enter
  horizontal posture. That uses 42% of scaled standing head height, or 55%
  when the lower body reports `Limited`. Use the effective threshold for
  this frame. Show which threshold is active and suppress both distances
  when the solver has no floor reference. A crossed threshold should remain
  visible when a dwell timer or movement state blocks the transition.
- **State readout.** Use three enlarged ASCII lines for posture and motion,
  floor height and source, and signed collider-base and head heights above
  that floor. Include the last live hit's age when a remembered floor is
  shown. Mark unavailable measurements with `--`. Use flight state directly
  and `Velocity.grounded` and world vertical speed to distinguish grounded,
  rising, and falling. Keep the posture thresholds visible as colored lines.

Use the snapshot that drove the body decision. The body solver reads
lower-body results before the next lower-body solve in the same update.
Capture the floor value, source, and entry/exit threshold at the decision so
the overlay can explain the frame that actually selected the posture.

## Surface highlight

Highlight one primary surface per selected avatar, chosen the same way as
the solver's floor reference. Draw a subdued box around the source collider
or collision mesh. On that surface, draw a brighter marking around the exact
support face. Retain the accepted hit's identity in the support query that
produced it. The current mesh query returns a height from its ray hit. For
mesh colliders, capture the winning triangle or a verified coplanar patch
when debug is enabled. If only a hit position is available, draw a point
marker there and keep the outer bounds.

Use a reusable planar bracket prefab for box faces or a bounded coplanar
patch. Four L-shaped corners can be built from eight thin cubes, with two
per corner and lengths fitted in the plane's local frame. For a triangle,
draw its actual edges or outline rather than suggesting a rectangular
contact area. Put the highlight in the existing overlay pass and pulse the
brighter markers between black and white on the overlay timer, so the
selected face reads against different materials. Keep the outer box steady.
When only a remembered floor is in use, show the last hit and its age as
historical data instead of highlighting it as a current hit.

## Implementation order and validation

1. Implemented. Capture the decision snapshot and draw support lines, thresholds, and the
   state readout. Reuse the overlay debug primitives and update retained
   geometry in place. Keep snapshot and drawing logic in a focused debug
   module. Leave solver decisions independent of debug drawing.
2. Implemented. Add the collider bounds and contact-face highlight. Resolve the exact hit
   from the existing query, without issuing an independent ground ray or
   scanning the mesh each frame. Reuse the corner prefab for rectangular
   faces and keep the extra work limited to selected avatars.
3. Headset check. On the roof, walk from a platform to lower ground while horizontal,
   stand up after losing contact, then mount and dismount. Verify the
   live-hit and remembered-floor markers before and after each transition.
   Repeat with a jump and flight, on a solid box face and on collider mesh
   geometry. Compare a local avatar with its peer view at normal and tiny
   heights, and verify the overlay clears when an avatar is removed.

Use the existing `CAT_DEBUG_BODY_SOLVER=1` trace while waiting for the overlay.
It already prints the mode, blend, head height, retained floor, scale, and
lower-body status. Record a trace before mounting and after dismounting if
the crawl state gets stuck again.

## Related work

- [Body solver](body-solver-horizontal-mode.md)
- [Third-person posture diagnostics](avatar-third-person-camera.md)
- [Walking IK at Mitten's pace](procedural-lower-body-walking-ik.md#walking-repro-from-sakura-twilight-simpler)

## Source boundaries

- Keep editor Settings row definitions, click handling, and context state in
  their existing entry points for this slice. A later split can move diagnostic
  toggle rows and dispatch together out of `internal/panels.mms` and
  `editor/settings_panel.rs`. Keep `editor/context.rs` responsible for active
  editor and selection state. Share the existing row factory and avoid
  per-frame panel rebuilds. Test Settings-only opening, selection, and toggles.
- Keep decision capture in `ik/body/mod.rs` and overlay rendering in its own
  module. If `system_world_update.rs` is split, move registration and phase
  orchestration by system family. Preserve the existing body/leg solve order,
  queue flushes, and phase timing. Disabled diagnostics must add no new scene
  queries or draw work.
- If `body_solver_scene_tests.rs` is split, keep the diagnostic lifecycle and
  threshold assertions with the posture fixtures. Reuse one loaded scene for
  live support, retained support, and toggle cleanup.
- A future split of `collidable_bvh.rs` can move retained surface geometry
  and support-hit data together. Keep the same BVH traversal and expose the
  winning triangle from that traversal. Validate box and mesh hits and verify
  that diagnostics add no collision queries for avatars with overlays off.
- If `lower_body/system.rs` grows further, move diagnostic snapshots and
  visualization to a dedicated module and keep one lower-body solve per
  avatar per frame. Validate the existing gait, grounding, and crawl tests
  with both debug toggles disabled and then enabled.
- Keep landing floor reacquisition with the lower-body support query and
  preserve separate knee-contact and posture-floor results. If the scheduler
  is split, reuse its existing BVH candidates for both. Validate stacked
  floors, a drop while crouched, and the case where no live floor is in reach.
  Cover Miku and other rigs with scene-owned collision. Keep explicit
  locomotion-target resolution cached and preserve the support-query bounds
  around that rig's current floor origin.
- If AvatarControl runtime fields are split, keep the solved posture and
  wearable mount binding with avatar pose state. Bind mounts when the mapped
  torso changes and use transform propagation for live alignment. Check avatar
  swaps, posture events, and wearer scale without adding per-frame lookups.
  Keep capsule inference with collision setup and prune worn equipment from
  model bounds so swapping avatars cannot change how their capsules are fitted.
- Keep manual capture records, paw meshes, and HUD lifecycle in the
  body diagnostics module. A future editor split can register all avatar
  diagnostic settings together, including capture HUD visibility. A scheduler
  split can group post-pose camera publication and camera-relative diagnostics.
  Keep mesh preparation lazy, use the existing solve snapshot, and validate
  capture clicks and HUD teardown through the Settings-only editor path.
