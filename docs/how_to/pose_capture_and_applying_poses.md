# Pose capture and applying poses

> Status: early-stage. The underlying pose-capture components and system exist, but the editor UI and authoring workflow are not fully implemented yet. Expect this guide and the APIs to expand.

Pose capture records the local transforms in an opted-in avatar subtree and stores them as a reusable pose. Applying a stored pose writes those transforms back to the matching nodes.

## Opt a model into pose capture

Attach `PoseCapture` beneath the glTF component whose spawned subtree should be captured:

```mms
GLTF.new("assets/models/avatar.glb") {
    EM.on()
    PoseCapture {
        label("Avatar")
    }
}
```

The label identifies the target in pose-capture UI. Pose capture is opt-in: models without this component are not presented as capture targets.

Current examples include:

- `examples/bisket-desktop-demo.mms`
- `examples/bisket-vr-only-example.mms`
- `examples/input-xr-gamepad.mms`

## Current data model

A capture target owns a pose library, and the library owns the captured poses:

```text
PoseCapture target
└── PoseCaptureLibrary
    ├── PoseCapturePose "pose 1"
    └── PoseCapturePose "pose 2"
```

Each pose stores local transform entries addressed relative to the capture target. This lets a pose be applied back to the corresponding nodes without depending on transient component IDs.

## Capturing and applying

The runtime supports `PoseCapture` and `PoseApply` intents. The developing pose panel is intended to expose these operations:

1. Select or locate a `PoseCapture` target.
2. Capture the current local transforms into its library.
3. Select a stored pose to apply it to that target.

At present, treat this as an experimental editor-assisted workflow rather than a stable MMS-only authoring interface. The panel may be incomplete, and serialized pose-library syntax is not yet presented here as a supported workflow.

## Interaction with live avatar control

Capture represents the transforms at the moment of capture. If `AVC`, `XRHand`, animation, constraints, or another system continues writing the same bones, it may immediately overwrite an applied pose. For predictable results, capture and apply when competing drivers are paused or scoped away from the affected bones.

Pose capture does not replace rig setup. Configure `AVC`, bone names, pole directions, and wrist corrections first; then use pose capture to preserve useful configurations.

## Planned expansion

This guide should grow when the UI workflow stabilizes to cover:

- naming and organizing poses;
- selecting a specific capture target;
- applying poses from MMS and events;
- serialization and loading pose libraries;
- combining poses with animation scopes;
- resolving conflicts with live IK and XR input.

For implementation design and unresolved details, see `docs/draft/pose-capture.md`.
