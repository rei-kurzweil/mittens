# Bug: Inspector Panel Duplicates Selected Subtree and Leaks Non-World Components

## Summary

After the world-panel payload routing fix, selecting some world-panel rows shows the correct subtree in the inspector, but the subtree is repeated a second time lower in the inspector list.

The same repro also shows component types in the inspector that do not appear in the world panel, such as `Bounds`.

## Reproduction

Use `examples/bisket-vr-demo.mms`.

1. Run the demo and open the editor panels.
2. In the first editor's world panel, click the root transform for the ground box.
   The current repro is the 2nd item in the first list for the first editor.
3. Observe the inspector panel.

Also repros with a root node shaped like:

```text
Transform
  Collision
    CollisionShape
```

1. Click the `Collision` row in the world panel.
2. Observe the inspector panel.

## Expected

- The inspector should render the selected authored subtree once.
- The subtree shown in the inspector should match the authored tree the world panel is presenting.
- Components intentionally hidden from the world panel should not unexpectedly appear in the inspector unless that is explicitly part of the inspector contract.

## Actual

- The inspector first shows the expected subtree, for example:
  - `collision`
  - `  collision_shape`
- Then the same subtree appears again below.
- Selecting the ground box root transform similarly repeats that authored branch.
- `Bounds` appears in the inspector even though it was not surfaced in the world panel listing.
- The world panel also shows duplicated status text:
  - one correctly sized status label inside the real status bar
  - and one oversized floating copy near the bottom, in front of the status area

## Current Suspicions

- The selected semantic target may now be correct, but inspector row generation may be traversing both:
  - the authored component subtree
  - and additional runtime-attached children or duplicated presentational children under the same target
- The duplicated world-panel status text suggests the same general class of issue may also be affecting panel rerender / attach / cleanup behavior for status content.
- The current `authored_scene_node_policy(...)` filtering in
  [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)
  may be insufficient for inspector-only traversal now that payload routing is selecting authored targets more reliably.
- `Bounds` likely indicates the inspector is traversing runtime helper components that the world panel scene model intentionally filtered out.

## Relevant Areas

- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)
  - `build_inspector_panel_model`
  - `build_inspector_panel_rows`
  - `push_inspector_panel_rows`
  - `authored_scene_node_policy`
  - `rerender_world_panel_status`
  - panel status attach / cleanup paths
- World-panel payload routing recently changed so inspector rebuilds now depend on semantic payload targets rather than clicked UI rows.

## Notes

- This regression appears after the world-panel selection payload fix, so it is probably in inspector traversal/filtering rather than world-panel row selection itself.
- The world panel still looks correct and selects visually as expected.
