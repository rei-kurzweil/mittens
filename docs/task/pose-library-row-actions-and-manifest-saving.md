# Pose library row actions and manifest saving

Status: implemented.

## Panel contract

Each `PoseCaptureComponent` is represented by one library section. The section model carries both the capture target ID and library ID.

The header contains:

- the configured label;
- `Capture`, targeting that section’s capture target;
- `Save`, targeting that section’s library.

Each pose row contains:

- a selectable row body with action `Select`;
- a separate button with action `Apply`.

All controls use `pose_panel_payload` data with explicit `Capture`, `Save`, `Select`, or `Apply` action text. Selecting a row remains local to `pose_capture_selection` and does not modify `EditorContextState.selected_component`.

The old global capture button is replaced by `pose_panel_status_value`.

## Apply contract

Apply resolves the destination in this order:

1. direct selected `GLTFComponent`;
2. a glTF whose `spawned_node_transforms` or `armature_joint_transforms` contains the selected component or one of its ancestors;
3. the original glTF owner of the pose library.

This recognizes imported primitive descendants, spawned transforms, joints, and armature marker descendants. A destination glTF does not need `PoseCaptureComponent`.

Preflight and execution both use the same validator. Every stored joint query must resolve exactly once within the destination glTF’s registered joint set. No `UpdateTransform` intents are emitted until all entries pass.

## Persistence contract

`PoseCapture.asset_name(string)` is optional for capture/apply and required for Save. Valid names are non-empty and contain only ASCII letters, digits, `_`, and `-`.

Save writes:

```text
assets/components/poses/<asset_name>/library.mms
assets/components/poses/<asset_name>/000-<sanitized-name>.pose.mms
assets/components/poses/<asset_name>/001-<sanitized-name>.pose.mms
...
```

Pose modules are written atomically in ECS child order. The manifest imports each module, calls each exported `pose()` in that order beneath one `PoseCaptureLibrary.new()`, and is atomically published after all modules succeed. Generated `NNN-*.pose.mms` files not referenced by the new manifest are removed after publication. Other files in the directory are preserved.

Rename, delete, reorder, dirty tracking, per-pose save, and import UI are out of scope.
