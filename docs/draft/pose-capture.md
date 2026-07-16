# Pose Capture

Design record for capturing authored or edited armature poses from imported glTF subtrees, storing them as reusable pose libraries, exposing them in editor UI, and assembling animation trees from captured poses.

The phase-1 capture, apply, panel, and standalone-library save workflow is implemented. Animation assembly remains future work. Where older exploratory alternatives remain below, the “Finalized phase-1 contract” section is authoritative.

## Finalized phase-1 contract

- `PoseCaptureComponent` has optional `label` and `asset_name` fields. `asset_name` accepts only ASCII letters, digits, `_`, and `-`.
- Each opted-in target owns one `PoseCaptureLibraryComponent`; poses are ordered child `PoseCapturePoseComponent` nodes.
- Every library header contains `Capture` and `Save`. Every pose row contains a separate `Apply` action.
- Row-body clicks only update the pose panel’s local selection/highlight.
- Apply prefers the glTF represented by `EditorContextState.selected_component`, recognizing direct glTF selections and descendants of registered spawned nodes or joints. This covers imported primitives and armature markers.
- If selection does not identify a glTF, Apply falls back to the pose’s original glTF owner.
- Every joint query must resolve exactly once before any update is emitted. Missing or ambiguous matches fail atomically.
- Save writes `assets/components/poses/<asset_name>/library.mms` plus ordered `NNN-<pose-name>.pose.mms` modules. Modules are replaced atomically, the manifest is published last, and stale generated modules are then removed.

## Goal

Support this loop across phases:

1. a glTF avatar is in the world
2. its armature transforms are posed manually or by tools
3. the user clicks a UI button to capture the current pose
4. the captured pose appears in a pose panel, grouped under that glTF
5. clicking a saved pose row reapplies the stored local transforms to that avatar
6. phase 2: the full pose list can later be turned into an `Animation {}` subtree
7. phase 4: pose lists should be serializable either:
   - as part of scene save, or
   - as standalone pose-library data without exporting the whole scene

## Current engine pieces we can build on

### glTF subtree structure is already usable

`GLTFSystem` already spawns each glTF node as its own named `TransformComponent` under the glTF anchor subtree, using the glTF node name when present and falling back to `node{index}`.

It also attaches:

- `BoneRestPoseComponent` to each spawned node as immutable local TRS at import time
- optional `SerializeComponent::off()` markers on spawned glTF nodes unless save-through is explicitly enabled

That means pose capture can treat the imported armature as:

- a stable local-transform tree
- with stable names inside one imported subtree
- plus a rest-pose sidecar for future diffing or reset behavior

### transform application already exists

Reapplying a pose does not need a new low-level mutation primitive. `TransformComponent` already updates through `IntentValue::UpdateTransform`, which carries local translation, rotation quaternion, and scale.

Pose application can therefore be implemented as a batched sequence of `UpdateTransform` intents targeted at the captured armature nodes.

### panel row payloads already exist

The editor panel helpers already build interactive rows with `DataComponent` payloads. The world panel and other editor panels already use this pattern for click-to-select behavior.

The pose capture panel should reuse the same model:

- section rows carry the glTF target
- pose rows carry the pose record identity
- add/capture buttons carry a panel action payload

### MMS subtree export already exists

The engine already has a clean subtree-to-MMS path via:

- `Component::to_mms_ast()`
- `subtree_to_ce_ast()`
- filtered world export

That matters for two reasons:

1. a pose library component subtree can be embedded in scene save without inventing a second serialization stack
2. phase 2 animation assembly can emit normal MMS component trees once that phase starts

## Proposed runtime concepts

## `PoseCaptureComponent`

Marker + configuration component attached to a glTF anchor or to a transform that owns a glTF subtree.

Purpose:

- opt a subtree into pose capture UI
- define which portion of the imported hierarchy counts as the captured armature
- make the target appear as a section in the pose capture panel

Suggested fields:

```rust
pub struct PoseCaptureComponent {
    pub label: Option<String>,
    pub asset_name: Option<String>,
    pub target_mode: PoseCaptureTargetMode,
    pub include_scale: bool,
    pub store_rest_deltas: bool,
}
```

Suggested target modes:

- `WholeSubtree`
- `SkinnedJointsOnly`
- `NamedRoot { selector_or_name }`

Initial recommendation:

- start with `WholeSubtree` over the glTF-spawned transform subtree
- filter later to armature-only capture once the exact joint-detection contract is settled

Reason: the engine already has the transform tree, but it does not yet expose a dedicated "these transforms are the skeleton bones for this instance" component query surface outside skin registration internals.

## `PoseCapturePoseComponent`

Serializable component representing one captured pose entry.

This should not live on every bone. It should live as one pose record containing an array of local transform samples for the target subtree.

Suggested shape:

```rust
pub struct PoseCapturePoseComponent {
    pub name: String,
    pub target_root_ref: PoseTargetRef,
    pub entries: Vec<PoseBoneEntry>,
}

pub struct PoseBoneEntry {
    pub path: String,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}
```

Where:

- `target_root_ref` identifies which captured glTF/armature this pose belongs to
- `path` is a durable bone key inside that subtree

## Bone identity: use subtree-relative path, not live `ComponentId`

`ComponentId` is runtime-only and should not be stored in pose data.

For pose entries, the stable key should be a subtree-relative path built from the imported node names, for example:

- `Armature/Hips/Spine/Chest/Neck/Head`
- `Armature/Hips/LeftUpperLeg/LeftLowerLeg/LeftFoot`

Why path keys are the safest first version:

- glTF nodes are already spawned with stable names
- save/load already preserves names in MMS output
- path lookup is local to one glTF subtree, so global uniqueness is not required
- this avoids needing imported-node GUID persistence immediately

Open issue:

- if a glTF has duplicate sibling names, path strings become ambiguous

If that appears in practice, switch the stored key to:

- `name path + sibling ordinal`, or
- glTF node index path captured at import time via a sidecar component

## `PoseCaptureLibraryComponent`

Container component holding a list of pose records for one target glTF subtree.

Suggested shape:

```rust
pub struct PoseCaptureLibraryComponent {
    pub target_root_ref: PoseTargetRef,
    pub poses: Vec<PoseCapturePose>,
}
```

This is the simplest scene-serializable representation if we want "poses grouped by glTF in the world" to exist as authored ECS state.

Alternative shape:

- one `PoseCaptureLibraryComponent` per target
- child `PoseCapturePoseComponent` nodes for individual poses

That child-node shape is probably better because:

- the panel can treat poses as rows backed by real components
- adding/removing/reordering poses fits existing subtree manipulation patterns
- animation assembly can reference individual pose nodes or duplicate them into generated animation content

Recommendation:

- use `PoseCaptureLibraryComponent` as the group marker
- store each pose as a child `PoseCapturePoseComponent`

## `PoseTargetRef`

Need a durable way to associate a library with a specific pose-capture target.

Suggested first version:

```rust
pub enum PoseTargetRef {
    Query(String),
}
```

Where the query resolves the `PoseCaptureComponent` root or owning glTF anchor.

For scene-local usage, a `#name` or authored query is likely enough.

Longer term, if users need pose libraries to survive retargeting or duplicated avatars, add:

- `Guid(uuid::Uuid)`
- `AssetInstance { gltf_uri, subtree_name }`

## Intent and signal flow

## New intent: `PoseCapture`

Add a user-facing intent variant:

```rust
IntentValue::PoseCapture {
    target: ComponentId,
    pose_name: Option<String>,
}
```

Meaning:

- resolve the `PoseCaptureComponent` / target subtree
- collect the current local transforms
- append a new `PoseCapturePoseComponent` under that target's library subtree

This should be the only required author-facing capture primitive.

## New intent: `PoseApply`

Add a second user-facing intent:

```rust
IntentValue::PoseApply {
    target: ComponentId,
    pose: ComponentId,
}
```

Meaning:

- read the stored pose entries
- resolve each entry path back into a live transform inside the target subtree
- emit `UpdateTransform` for every matched transform

This keeps "capture" and "apply" explicit and mirrors the existing intent-driven model.

## Optional event: `PoseCaptured`

Not required for the first version, but useful if other panels or tools need to react:

```rust
EventSignal::DataEvent {
    name: "pose_captured".to_string(),
    payload: Some(pose_component_id),
}
```

The engine already has generic `DataEvent`, so a dedicated event variant is probably unnecessary.

## `PoseCaptureSystem`

New system responsible for:

1. discovering `PoseCaptureComponent` targets
2. ensuring each target has a pose-library subtree
3. handling `PoseCapture` intent
4. handling `PoseApply` intent
5. optionally generating animation trees from captured poses

This system should own the capture/apply logic rather than putting it in panel code.

The panel should only emit intents.

## Capture algorithm

Capture is selection-aware at the editor-workspace level:

1. if an editor currently has a gizmo-attached selection inside a `PoseCaptureComponent` subtree,
   capture only that pose-capture target
2. otherwise, capture one pose into every discovered `PoseCaptureComponent` target in the world

For each resolved `PoseCaptureComponent` target:

1. resolve or create exactly one `PoseCaptureLibraryComponent` child for that target
2. resolve the root transform subtree to capture
3. walk descendants in deterministic order
4. for each relevant transform:
   - read its current local TRS from `TransformComponent`
   - compute its subtree-relative path
   - append a `PoseBoneEntry`
5. create a new `PoseCapturePoseComponent`
6. attach it under that target's pose-library subtree

Deterministic order matters so:

- serialized output is stable
- diffing pose captures is readable
- animation assembly produces predictable ordering

Recommended order:

- depth-first traversal in topology order

## Apply algorithm

For a given target + pose:

1. build or reuse a map from subtree-relative path to live transform component
2. iterate stored pose entries
3. for each matched live transform:
   - emit `IntentValue::UpdateTransform`
4. skip unmatched entries with a warning

Important: pose apply should write local transforms only.

This matches capture semantics, imported glTF local TRS, animation keyframe expectations, and current transform mutation APIs.

## Panel design

## `pose_capture_panel`

Add a new editor panel section similar in role to world/paint/grid panels.

The panel groups captured content by pose library in the current world, with one
library per `PoseCaptureComponent` target.

Section model:

- library header: target/library label, `Capture`, and `Save`
- pose row: selectable label body and explicit `Apply`
- compact panel status bar

Click behavior:

- `Capture` emits `PoseCapture { target, pose_name: None }` for that header’s target
- the pose row body only updates panel-local selection
- `Apply` resolves the current editor/visual glTF selection, preflights every joint, then emits one `PoseApply` intent for the resolved glTF
- `Save` synchronously writes the complete library and reports success or failure in the panel status

## Why this should be driven by `PoseCaptureComponent`

The user asked for glTF components to become sections in the editor only when they opt in to pose capture. That implies the panel should not enumerate every `GLTFComponent` blindly.

Instead:

- `GLTFComponent` continues to mean "spawn this asset"
- `PoseCaptureComponent` means "also expose this spawned subtree in the pose capture UI"

That keeps capture/editor semantics explicit and avoids cluttering the panel with every incidental imported asset.

## Panel payload model

Use the existing `DataComponent` row payload pattern.

Final payload keys:

- `action = "Capture" | "Save" | "Select" | "Apply"`
- `target_component = <PoseCapture root>`
- `library = <PoseCaptureLibrary>`
- `pose = <PoseCapturePose>` for Select and Apply

The stopgap editor panel adapter can decode these payloads the same way it already decodes world/grid/panel actions.

## Selection behavior

The pose capture panel should not overload scene selection.

Final behavior:

- clicking a pose row highlights it in the panel-local `SelectionComponent`
- selection does not apply the pose and does not replace editor/glTF selection
- only the explicit `Apply` control applies a pose

## Phase 2: Animation assembly

This is explicitly not phase 1 work for pose capture.

## Goal

Support building an animation component tree from all poses in a library.

The simplest first output is:

```text
AnimationComponent
  KeyframeComponent(beat=0)
    ActionComponent(UpdateTransform for bone A)
    ActionComponent(UpdateTransform for bone B)
    ...
  KeyframeComponent(beat=1)
    ActionComponent(...)
```

Each captured pose becomes one keyframe.

## Why Action + UpdateTransform fits well

The existing animation system already knows how to:

- register animations and keyframes
- fire `ActionComponent`
- carry `IntentValue::UpdateTransform` through actions
- serialize those actions back to MMS via `to_mms_ast`

So pose-to-animation assembly does not need a new animation runtime.

It only needs a builder that translates stored pose data into:

- one `AnimationComponent`
- one `KeyframeComponent` per pose
- many `ActionComponent::new_authored(IntentValue::UpdateTransform { ... })`

## Target references inside generated actions

Generated `ActionComponent`s should not hardcode live `ComponentId`s if the animation is meant to serialize cleanly.

They should be authored using unresolved target references, matching how normal authored actions work.

That means each generated transform action should carry:

- a placeholder `UpdateTransform { component_ids: vec![ComponentId::null()], ... }`
- `target_sources = vec![ComponentRef::Query(path_selector)]`

Open question:

- do we want the selector form to be a simple `#name` chain or a dedicated path selector rooted at the avatar subtree?

Recommendation:

- add a pose-specific subtree path resolution helper rather than relying on globally ambiguous `#name`

The pose library already stores path strings, so the animation assembler should preserve those same path strings as the authored target sources.

That implies one missing piece:

- `ComponentRef::Query` resolution today searches from world roots, not from a local avatar root

So if generated animation actions are meant to be reusable and local to an avatar subtree, we likely need either:

1. a new query form that is resolved relative to the animation root or target root, or
2. an animation-local target binding component that maps pose paths to live component ids before actions fire

This is the main missing seam for clean animation generation.

## Recommended first implementation for animation assembly

Do not solve generalized query scoping yet.

Instead:

1. build the animation subtree as a child of the same `PoseCaptureComponent` target
2. materialize `ActionComponent`s with already-resolved live `ComponentId`s
3. accept that this first version is scene-local and not yet fully retargetable

Then later, once local query scoping exists, switch generation to durable `ComponentRef`-based actions.

This keeps the feature moving without blocking on a broader selector redesign.

## Phase 4: Serialization

This is explicitly later than initial capture/apply and later than phase 2 animation generation.

## Scene-embedded save

This is the easy case.

If pose library subtrees are attached under the authored scene and marked serializable, existing scene export should include them as normal MMS components.

This argues for pose libraries being real components/subtrees, not purely in-system ephemeral arrays.

## Standalone pose-library save

The user also wants to save pose lists independently of the full scene.

That should be treated as a distinct export mode, not as a special case of full world save.

Implemented shape:

```text
assets/components/poses/<asset_name>/
├── library.mms
├── 000-idle.pose.mms
└── 001-step_left.pose.mms
```

Each pose module exports `pose()`. The manifest imports every module in ECS child order and materializes one `PoseCaptureLibrary`.

This is separate from animation export on purpose:

- a pose list is a library of snapshots
- an animation is an ordered playback structure

They are related, but not identical authoring artifacts

## Internal arrays vs ECS subtree storage

The original request suggested storing captured transforms as arrays internally in `pose_capture_system`.

Recommendation:

- use arrays as the system's working format
- persist them as ECS components/subtrees

Reason:

- arrays are the natural capture/apply runtime representation
- ECS subtree storage is the natural editor/serialization representation

That gives both:

- efficient in-system processing
- visible, inspectable, serializable authored data

## Missing pieces

These are the main gaps that are not fully solved by current engine pieces.

### 1. target filtering for "armature only"

Current glTF import clearly spawns the full node tree, but pose capture needs a precise rule for which transforms belong to the armature.

Possible policies:

- all descendant transforms under the pose-capture root
- only transforms referenced by the active skin joint table
- only transforms under a named armature root

Recommendation:

- start with all descendant transforms
- add `SkinnedJointsOnly` once the joint-resolution surface is exposed cleanly from glTF/skinned-mesh runtime state

### 2. subtree-relative path resolution helper

Pose apply and standalone animation generation both want the same thing:

- resolve `Armature/Hips/Spine/...` against one known target root

That should be a shared helper in the pose capture system or world-query layer.

### 3. panel integration point

There is already an editor stopgap MMS/panel adapter. The pose capture panel can likely follow the same pattern as grid/world panels, but it still needs:

- a panel model builder
- click payload decoding
- rerender on capture/add/remove/apply-relevant state changes

### 4. pose naming UX

The first version can auto-name poses:

- `pose_001`
- `pose_002`

Later we can add rename support in the panel.

### 5. standalone import UI

Standalone export is implemented by the library-header Save action. Import UI remains future work.

## Recommended implementation order

1. add `PoseCaptureComponent`
2. add `PoseCaptureLibraryComponent` + `PoseCapturePoseComponent`
3. add `PoseCapture` and `PoseApply` intents
4. implement `PoseCaptureSystem` capture/apply using subtree-relative path arrays
5. add `pose_capture_panel` with grouped sections and per-library Capture/Save plus per-pose Apply
6. phase 2: add animation assembly from pose lists
7. phase 2+: improve animation target scoping for reusable generated animations
8. phase 4: mark pose libraries scene-serializable and verify full scene export
9. add "export pose library subtree to MMS" as a separate operation (implemented)

## Recommendation summary

The cleanest first version is:

- opt-in via `PoseCaptureComponent`
- store captured poses as ECS-authored pose-library subtrees
- represent each pose as an array of local TRS entries keyed by subtree-relative path
- trigger capture and apply through explicit intents
- expose the data through a dedicated `pose_capture_panel`
- use existing scene export for embedded serialization
- add standalone pose-library export as a separate helper, not as a scene-save variant

The main architectural caution is animation generation: applying poses to a live avatar is straightforward, but generating reusable serialized animation trees wants subtree-local target resolution, which the current global `ComponentRef::Query` path does not provide yet.
