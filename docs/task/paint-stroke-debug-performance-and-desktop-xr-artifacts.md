# Paint-stroke debug performance and desktop/XR artifacts

Date: 2026-08-20

Status: discovery tracker; do not treat panel minimization as a fix

Related:

- [Grid-aware paint stroke interaction model](grid-aware-paint-stroke-interaction-model.md)
- [Editor selection and Paint performance](editor_selection_and_paint_perf.md)
- [Transform gizmo minimum screen extent](transform-gizmo-minimum-screen-extent.md)
- [Gizmo screen size varies with camera distance](../bugs/transform-gizmo-screen-size-varies-with-camera-distance.md)
- [XR hand laser selectable and origin is past fingertip](../bugs/xr-hand-laser-is-selectable-and-origin-is-past-fingertip.md)

## Reported checkpoint

`paint-stroke-debug` runs near 30 FPS in desktop/non-OpenXR use even with the
World and Assets panels minimized. `vtuber-desktop`, despite being a broadly
similar desktop editor scene, runs near 60 FPS.

The diagnostic scene also shows:

- XR-style laser geometry resting on the grey floor during desktop use; and
- transform gizmos whose apparent size depends strongly on selected-object
  position/distance, but does not update as the desktop camera approaches.

These are separate from the now-fixed procedural Paint preview-instantiation
failure. They should be measured and fixed independently of the grid gesture
model.

## Source inventory and leading hypotheses

`examples/paint-stroke-debug.mms` differs materially from `vtuber-desktop.mms`:

1. It always declares `InputXR.on()` plus left and right
   `XRHand(...).laser()` rigs, even when launched in desktop mode. This is
   the first source-level explanation for the stray laser and a performance
   suspect: verify whether it starts, polls, or renders XR work without an
   active OpenXR session.
2. It creates an editor UI with World, Assets, Paint, Color, Grid, and Settings
   panels, two grids, three wall targets, and two large floor/shelf targets.
   Minimized panels may still participate in layout, render extraction, BVH,
   event routing, and editor refresh work; hidden visual content alone does
   not prove their systems are inactive.
3. It exercises camera-specific gizmo scaling. Existing tracking says that the
   scale is derived from camera-space depth but can become stale or lack a
   projected-space minimum. The reported near-camera non-resize behavior fits
   that open bug/task rather than a Paint-specific transform error.

None of these is yet a measured root cause for the 30-FPS cap. In particular,
first establish whether the limit is CPU frame work, GPU/vsync pacing, OpenXR
frame pacing, or a fixed render-present rate before optimizing a subsystem.

## Investigation plan

- [ ] Record baseline frame time, CPU/GPU timing, present/vsync behavior, and
      active OpenXR-session state for both examples at the same window size.
- [ ] Run an A/B desktop diagnostic scene with the `InputXR`/`XRHand.laser()`
      subtree absent, then compare FPS and verify the floor laser disappears.
- [ ] Independently remove or suspend editor panels, grids, and scene targets
      to isolate layout/editor-refresh/BVH/render cost. Do not infer results
      from panel minimization alone.
- [ ] Use the existing spatial and XR profiling hooks where applicable; add
      frame-stage timings only where current hooks leave an unexplained gap.
- [ ] Verify whether the desktop launcher creates an OpenXR session merely
      because the scene contains `InputXR.on()`.
- [ ] Reproduce gizmo scaling at near/far target depth while moving the camera;
      capture the camera family, calculated scale, visible scale, and BVH scale
      in the same frame.
- [ ] Route the gizmo finding to the existing screen-extent task unless the
      trace proves a separate Paint/debug-scene lifecycle bug.

## Exit criteria

- The 30-FPS behavior has a measured limiter and a before/after comparison.
- Desktop runs do not render or expose XR laser artifacts unless an XR session
  is active and the diagnostic explicitly asks for them.
- Gizmo visible and interactive scale update together as the active desktop
  camera moves, with the existing minimum-screen-extent decision documented.
- The diagnostic scene remains useful for Paint/grid tests without silently
  adding unrelated XR or editor-frame workload.
