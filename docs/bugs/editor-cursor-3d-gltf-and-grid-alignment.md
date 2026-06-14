# Bug: Editor 3D Cursor GLTF Coverage and Grid Alignment

Date: 2026-06-13

## Summary

There are currently two separate issues in the editor `3D Cursor` workflow:

1. In `bisket-vr-demo`, the `3D Cursor` only appears when clicking the Bisket model.
   Other scene objects, such as the glowing animated cubes, still work in `Select`
   and `Select + Cursor`, but do not show the cursor in `3D Cursor` mode.
2. When the cursor is placed near Bisket's feet, the cursor visual appears level,
   but `Add Grid` creates a grid that is noticeably tilted relative to the cursor's
   apparent orientation.

These should be treated as separate bugs.

## Bug 1: 3D Cursor Only Appears on Bisket in `bisket-vr-demo`

### Repro

1. Open `bisket-vr-demo`.
2. Switch the editor interaction mode to `3D Cursor`.
3. Click the Bisket model.
4. Observe that the cursor appears.
5. Click other scene objects, such as the glowing animated cubes.
6. Observe that no cursor appears.

### Current observations

- The same non-Bisket objects can still be interacted with in `Select` mode.
- They also work in `Select + Cursor`, which suggests the click / raycast /
  drag-start path is alive for those objects.
- In contrast, pure `3D Cursor` mode does not show the cursor for those same
  non-Bisket objects.
- GLTF content also appears to have an asymmetry in default clickability:
  the GLTF itself does not seem clickable by default, while bone markers do,
  presumably because those markers are explicitly raycastable.

### Notes

- `Selectable` should not be the deciding factor for `3D Cursor` placement.
- The likely distinction here is raycastability / resolved target renderable /
  surface-alignment behavior, not selection gating.

### Likely debugging threads

- Compare the clicked renderable / resolved target renderable for Bisket versus
  the animated cubes in `Cursor3dSystem`.
- Check whether the cubes resolve a scene hit but fail marker placement later.
- Audit whether those non-Bisket renderables are missing something needed by
  `resolve_surface_aligned_pose_for_subtree(...)`.
- Confirm current GLTF default behavior:
  - whether the GLTF subtree itself is skipped from editor auto-raycast wrapping
  - whether only helper markers are explicitly raycastable
  - whether this causes inconsistent `3D Cursor` coverage versus ordinary select

## Bug 2: Grid Placement Does Not Match Cursor Orientation

### Repro

1. Open `bisket-vr-demo`.
2. Switch to `3D Cursor`.
3. Click near Bisket's feet or just above them so the cursor appears.
4. Observe that the cursor looks visually level:
   - it appears horizontally flush
   - `Y` appears up
5. Use `Add Grid`.
6. Observe that the created grid is tilted by roughly 10 to 20 degrees along
   `X` and `Z`, instead of matching the cursor's apparent orientation.

### Current observations

- The cursor marker itself appears visually aligned in a way that suggests a
  level placement.
- The spawned grid does not agree with that apparent orientation.
- This implies one of:
  - the cursor visual is misleading relative to the stored cursor rotation
  - the stored cursor rotation is correct, but `Add Grid` interprets it
    differently
  - the grid is being created from a different basis / transform than the one
    shown by the cursor marker

### Likely debugging threads

- Log the exact stored cursor rotation used after placement.
- Log the transform used by `Add Grid` when creating the grid.
- Compare the grid spawn rotation directly against
  `EditorContextState.cursor_rotation`.
- Verify whether the cursor marker's three planes visually communicate the same
  basis as the one used for spawned content.
- Check for quaternion basis conversion or local/world-space mismatch when grid
  creation consumes the cursor pose.

## Expected behavior

- `3D Cursor` mode should work consistently across editor-scene objects that are
  otherwise interactable via editor clicking.
- GLTF-backed content should have predictable default behavior for cursor
  placement and selection.
- Objects spawned from the cursor, including grids, should use the same pose the
  cursor visual communicates to the user.
