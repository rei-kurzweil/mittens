# Pose capture and applying poses

Pose capture records local joint transforms from an opted-in glTF instance, presents the resulting library in the editor, and can apply a stored pose to another compatible instance.

## Opt a model into pose capture

Attach `PoseCapture` beneath the glTF component:

```mms
GLTF.new("assets/models/avatar.glb") {
    EM.on()
    PoseCapture {
        label("Avatar")
        asset_name("avatar")
    }
}
```

`label` is the library header shown in the pose panel. `asset_name` is optional for capture and apply, but required by Save. It may contain only ASCII letters, digits, `_`, and `-`.

Models without `PoseCapture` do not appear in the panel.

## Use the pose panel

Each opted-in target has one library header:

- `Capture` records joints whose local transform differs from the glTF import/rest pose.
- `Save` writes the complete library to disk.

Each pose has a row body and an `Apply` button:

- Clicking the row body only selects and highlights that row inside the pose panel.
- Clicking `Apply` applies the pose. It does not replace the editor or scene selection.

Capture names are generated as `pose_0`, `pose_1`, and so on. Rename, delete, reorder, and per-pose save are not currently part of the panel workflow.

## Apply target selection

Apply first tries to identify the glTF instance represented by the current visual/editor selection. A direct glTF selection works, as do selections on:

- imported mesh primitives;
- imported spawned nodes;
- armature joints;
- armature visualization markers.

This allows a pose captured from one instance to be applied to another compatible instance. The destination does not need its own `PoseCapture` component.

If the current selection is unrelated to a glTF instance, Apply falls back to the glTF that originally owned the pose.

Compatibility currently means every stored joint query must match exactly one joint in the destination instance. Validation is atomic: if any query is missing or ambiguous, no transforms are applied and the panel status reports the failure.

## Saved files

Save writes the full library under:

```text
assets/components/poses/<asset_name>/
├── library.mms
├── 000-<pose-name>.pose.mms
├── 001-<pose-name>.pose.mms
└── ...
```

Pose names are sanitized for filenames. The numeric prefix preserves ECS child order.

Each pose module exports one `pose()` function. `library.mms` imports every generated module in order and materializes one `PoseCaptureLibrary`. Pose modules are replaced atomically, the manifest is published last, and stale generated pose modules are removed after the new manifest is in place.

## Interaction with live avatar control

Capture omits untouched rest-pose joints, so applying a pose does not reset unrelated bones such as an AVC-owned head joint. A joint that is actively moved away from its imported rest pose by `AVC`, `XRHand`, animation, constraints, or another system is still considered changed and will be captured. Those systems may also immediately overwrite an applied pose, so pause or scope competing drivers when a persistent applied pose is required.

Pose capture does not replace rig setup. Configure bone names, pole directions, and wrist corrections first, then capture useful configurations.
