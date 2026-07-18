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
- `Reset` restores every armature joint with imported rest-pose metadata before applying another pose.
- `Save` writes the complete library to disk.

Each pose has an editable name and an `Apply` button. Renaming marks the library unsaved; the next `Save` renames the existing numbered pose module and rewrites the manifest without retaining a duplicate file.

- Clicking the row body only selects and highlights that row inside the pose panel.
- Clicking `Apply` applies the pose. It does not replace the editor or scene selection.

Capture names are generated as `pose_0`, `pose_1`, and so on. Delete, reorder, and per-pose save are not currently part of the panel workflow.

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

## Declarative startup poses

Place a pose directly inside a `GLTF` to overlay it automatically once that model's imported nodes and armature joints are initialized:

```mms
let relaxed = relaxed_pose_factory()

let avatar = GLTF.new("assets/models/avatar.glb") {
    relaxed
}
```

Direct pose children are applied once per successful glTF spawn, in ECS child order. Each uses overlay semantics, so it changes only its captured joints; when multiple startup poses contain the same joint, the later child wins. Each pose is validated atomically and independently: a missing or ambiguous joint prevents that pose from writing any transforms, but does not prevent later startup poses from applying.

Only immediate `PoseCapturePose` children have startup behavior. Indirect descendants, poses stored under a `PoseCaptureLibrary`, animation keyframes, and poses attached after the model has spawned are not applied automatically. Use explicit `apply`, `overlay`, or `apply_blended` calls for those cases and for any pose that should be reapplied or animated.

## Interaction with live avatar control

Capture omits untouched rest-pose joints, but the application mode determines what happens to omitted joints:

- `pose.apply(target)` uses replace mode. It first restores every imported joint to its glTF rest transform, then writes the captured joints. This makes sparse poses deterministic and restores joints omitted by the next pose.
- `pose.overlay(target)` uses sparse layering. It writes only captured joints and leaves every omitted joint unchanged.
- `pose.apply_blended(target, amount)` blends every imported joint from rest toward the captured pose; omitted joints stay at rest.

A joint that is actively moved away from its imported rest pose by `AVC`, `XRHand`, animation, constraints, or another system is considered changed and can be captured. Those systems may also overwrite an applied pose, so pause or scope competing drivers when a persistent applied pose is required.

Pose capture does not replace rig setup. Configure bone names, pole directions, and wrist corrections first, then capture useful configurations.
