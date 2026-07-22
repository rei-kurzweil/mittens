# Secondary Motion Panel and Directional Armature Highlighting

Date: 2026-07-22

Status: proposed

## Goal

Implement the phase-1 secondary-motion inspection panel defined in
[`secondary_motion_panel.mms.md`](../draft/secondary_motion_panel.mms.md), including targeted
highlighting of the imported bone represented by a `SpringJointComponent`.

As part of that work, upgrade armature visualization markers from cubes to directional cones and
support in-place per-joint highlight color changes without rebuilding the armature visualization.

## Why this is one task

The panel can list authored `SpringJointComponent`s without visualization changes, but its core
interaction is only useful if a user can identify the corresponding imported bone in the scene.

That interaction crosses three existing ownership boundaries:

```text
SecondaryMotionSystem
  owns the authoritative joint-config -> imported-transform binding

secondary-motion panel controller
  owns rows, panel-local selection, and click semantics

ArmatureVisualizationSystem
  owns marker helper nodes and their visual state
```

The implementation should connect those owners through narrow projections/operations. It should
not duplicate selector resolution in the panel or expose either system's runtime maps.

## Strategic signal guidance

This task should not add a family of panel-specific lifecycle signals.

Reuse existing facts and lifecycle intents:

- `RegisterSecondaryMotion` for newly visible roots/chains/joints;
- `ParentChanged` -> `SecondaryMotionTopologyChanged` for hierarchy changes;
- `SecondaryMotionConfigurationChanged` for live authored-field edits;
- `GltfInitialized` -> `SecondaryMotionGltfInitialized` for binding readiness/respawn;
- current subtree cleanup / `UnregisterSecondaryMotion` for removal;
- `ResetSecondaryMotion` for explicit rebinding.

Those causes may mark the panel model dirty through one shared refresh path.

At most one new state-setting operation is expected for the click-to-highlight ownership crossing:

```rust
SetSecondaryMotionJointHighlight {
    joint_config: Option<ComponentId>,
}
```

Before adding it, verify whether the panel controller can safely receive narrow access to the
secondary-motion and armature-visualization entry points. Prefer a direct call when the ownership
boundary already permits it. Use the intent only when RX/mutation-executor dispatch is required.

Do not create separate variants for highlight, unhighlight, ensure-visible, marker-refresh, and
panel-refresh.

## Current implementation inventory

### Pose panel pattern

`src/engine/ecs/system/editor/pose_panel.rs` already provides:

- explicit model/section/row structs;
- `DataRendererSystem` list reconciliation;
- Rust-rendered dynamic rows inside an MMS-owned stable shell;
- payload-backed click decoding;
- panel-local `SelectionComponent` behavior;
- a status bar and empty-state behavior.

The secondary-motion panel should reuse this structure but should not inherit pose capture,
application, or library persistence semantics.

### Armature visualization

`ArmatureVisualizationSystem` currently:

- tracks marker roots per GLTF as `HashMap<ComponentId, Vec<ComponentId>>`;
- spawns one cube marker as a child of each imported joint transform;
- colors every marker white;
- tears down/recreates markers only for visibility changes;
- has no joint-to-marker reverse lookup and no highlight state.

The built-in cone mesh already exists and points along local `+Z`.

### Secondary-motion retained runtime

`SecondaryMotionSystem` already retains:

- roots and owning GLTFs;
- child-to-root ownership;
- joint-config-to-chain ownership;
- bound chain joint configs and exact imported transform ids;
- waiting/invalid diagnostics;
- imported-transform dependency indexes.

That is the authoritative source for the panel snapshot and highlight resolution.

## Implementation stages

### Stage 1 — read-only secondary-motion inspection projection

- [ ] Add public read-only DTOs for roots, chains, binding status, joint configs, and resolved
      imported transforms.
- [ ] Add a narrow `SecondaryMotionSystem` snapshot method.
- [ ] Keep runtime maps private.
- [ ] Preserve authored joint ordering.
- [ ] Define deterministic root and chain display ordering independent of `HashMap` order.
- [ ] Include waiting/invalid messages without exposing mutable runtime state.
- [ ] Add unit tests for bound, waiting, invalid, overlapping, and removed chains.

The snapshot is built on panel refresh, not during every frame tick.

### Stage 2 — panel shell and model

- [ ] Add `secondary_motion_panel(...)` to `assets/components/panels.mms` using the pose-panel shell
      conventions.
- [ ] Add `src/engine/ecs/system/editor/secondary_motion_panel.rs`.
- [ ] Define root-section, chain-subsection, and joint-row model types.
- [ ] Render vertically separated root sections.
- [ ] Render chain headers and ordered joint rows.
- [ ] Add an empty state.
- [ ] Add compact waiting/invalid presentation and status-bar detail.
- [ ] Add the panel kind to the existing panel mounting/selection infrastructure.
- [ ] Add stable selectors and payload names for tests and click decoding.

### Stage 3 — panel refresh lifecycle

- [ ] Mark/reconcile the panel model after relevant existing secondary-motion lifecycle signals.
- [ ] Coalesce multiple lifecycle causes in one drain into one panel refresh.
- [ ] Do not refresh from the secondary-motion frame tick.
- [ ] Do not refresh merely because armature marker transforms changed.
- [ ] Confirm unrelated `ParentChanged` events do not rebuild panel content.

If the current stopgap adapter makes targeted dirtying awkward, use its existing shared panel refresh
temporarily and record the narrower `PanelSystem` integration as follow-up. Do not add one signal per
refresh cause.

### Stage 4 — directional cone markers

- [ ] Replace `RenderableComponent::cube()` with cone segment rendering.
- [ ] Derive armature edges from the GLTF's recorded armature joint transforms and ECS parent/child
      relationships during marker creation.
- [ ] For each parent-to-child armature edge, orient cone local `+Z` along the child's local
      translation.
- [ ] Place the cone midpoint at half the edge vector.
- [ ] Scale cone Z to edge length and X/Y to a stable marker radius.
- [ ] Define deterministic behavior for branching joints.
- [ ] Map leaf joints to their incoming segment for highlighting.
- [ ] Preserve overlay, raycast, selection, and transform-routing behavior.
- [ ] Verify visualization never changes the imported joint transforms.

Do not recompute cone topology every frame. Marker transforms inherit from the imported joints after
spawn.

### Stage 5 — semantic marker registry

- [ ] Replace anonymous per-GLTF marker vectors with a structure that maps imported joint ids to
      marker roots and color components.
- [ ] Retain per-GLTF highlighted joint state.
- [ ] Retain a pending highlighted joint when visualization is hidden or not yet spawned.
- [ ] Add a narrow `set_highlight(gltf, joint: Option<ComponentId>)` entry point.
- [ ] Restore the old joint's normal color and update the new joint's orange color through existing
      color mutation/visual update paths.
- [ ] Make repeated selection of the same joint idempotent.
- [ ] Ensure hide/show cycles restore the pending highlight without respawning for color-only
      changes.

Highlight color target:

```rust
const ARMATURE_NORMAL_RGBA: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const ARMATURE_HIGHLIGHT_RGBA: [f32; 4] = [1.0, 0.48, 0.08, 1.0];
```

### Stage 6 — joint-row interaction

- [ ] Add a `SelectJoint` payload to clickable joint rows.
- [ ] Update panel-local selection without changing world/editor selection.
- [ ] Resolve the imported joint through the retained secondary-motion snapshot/runtime.
- [ ] For a bound joint, ensure the owning GLTF armature visualization is visible.
- [ ] Set the imported joint highlight through the narrow armature visualization operation.
- [ ] For waiting/invalid joints, keep row selection and show status without disturbing the current
      highlight.
- [ ] Clear highlight when panel selection is explicitly cleared or the selected config is removed.

If an intent is required, add only the single state-setting operation described above and dispatch
it centrally through the mutation executor.

### Stage 7 — cleanup and documentation

- [ ] Remove marker highlight ownership when a GLTF or imported joint is removed.
- [ ] Remove panel selection when its joint config disappears.
- [ ] Confirm subtree cleanup removes marker helpers and panel-generated content through canonical
      paths.
- [ ] Update the signal guide only if a new intent is actually introduced.
- [ ] Update editor/panel documentation with how to open and use the panel.
- [ ] Record any remaining dependency on the panel-system/stopgap-adapter migration.

## Geometry contract

For a cone mesh centered on local Z in `[-0.5, +0.5]`, an edge vector `v` in the parent joint's
local space produces:

```text
translation = v * 0.5
rotation    = shortest_arc(+Z, normalize(v))
scale       = [marker_radius, marker_radius, length(v)]
```

This makes the cone base coincide with the parent joint and the tip coincide with the child joint.

Near-zero-length edges must be skipped or represented by a small deterministic fallback marker;
they must not create NaN rotations.

For non-uniform inherited scale, document and test the chosen behavior. The marker must not mutate
or normalize the authored/imported joint transform to compensate.

## Test plan

### Panel model

- [ ] no roots produces the empty state;
- [ ] multiple roots produce vertically separate sections;
- [ ] duplicate chain names remain scoped under their roots;
- [ ] joint rows preserve authored order;
- [ ] query and GUID references render correctly;
- [ ] bound/waiting/invalid statuses project correctly;
- [ ] removed and reparented components disappear/move after lifecycle refresh;
- [ ] repeated refresh produces deterministic item ordering and stable keys.

### Interaction

- [ ] clicking a bound joint selects only the panel row;
- [ ] the corresponding imported marker becomes orange;
- [ ] selecting another joint restores the prior marker to white;
- [ ] selecting the same joint twice is idempotent;
- [ ] clicking an unresolved joint does not clear or misdirect the current highlight;
- [ ] removing the selected config clears selection/highlight safely;
- [ ] panel interaction does not attach a gizmo or change world-panel selection.

### Armature visualization

- [ ] markers spawn once and visibility removal remains idempotent;
- [ ] markers use cone renderables, not cubes;
- [ ] cone base/tip align with parent/child joints for X, Y, Z, and diagonal edges;
- [ ] branching and leaf mappings are deterministic;
- [ ] color-only highlight changes do not remove or recreate marker roots;
- [ ] hide/show preserves the pending highlighted joint;
- [ ] GLTF respawn cleans stale marker ids and restores highlight when the target still exists;
- [ ] imported joint world/local transforms are unchanged by visualization.

### Performance

- [ ] steady-state secondary-motion ticks do not build panel snapshots;
- [ ] steady-state armature visualization does not enumerate the whole ECS world;
- [ ] highlighting performs retained lookups plus two color updates, not marker-tree rebuild;
- [ ] unrelated topology events do not rerender secondary-motion panel content;
- [ ] many unrelated ECS components do not change panel or highlight work.

## Acceptance criteria

- [ ] A mounted Secondary Motion panel lists every initialized root, chain, and joint configuration.
- [ ] Root and chain hierarchy is visually clear in one vertical scroll view.
- [ ] Clicking a bound joint highlights its actual retained imported transform.
- [ ] Highlighting is medium-bright orange/yellow-red and visibly distinct from white markers.
- [ ] Selecting another joint updates colors without rebuilding the armature visualization.
- [ ] Armature markers are directional cones whose orientation communicates bone direction.
- [ ] Branching and leaf joints remain highlightable.
- [ ] Waiting/invalid bindings remain visible and safe to click.
- [ ] The feature introduces no per-frame world scan, selector resolution, or panel rerender.
- [ ] The implementation adds no more than one new semantic highlight intent, and only if required
      by the ownership boundary.

## Non-goals

- editing stiffness, drag, gravity, center, references, or virtual endpoints;
- adding/deleting/reordering secondary-motion components from the panel;
- multiple simultaneous highlighted joints;
- animation of highlight colors;
- changing the authored GLTF armature;
- replacing the general panel-system migration;
- redesigning all editor selection semantics.

## Related documents

- [Secondary Motion Panel MMS Draft](../draft/secondary_motion_panel.mms.md)
- [Secondary-motion signal review](../review/secondary_motion_signals.md)
- [Secondary-motion runtime specification](../spec/secondary_motion_system.md)
- [Pose capture draft](../draft/pose-capture.md)
- [Panel model/view contract](../draft/panel-model-view-contract.md)
- [Armature visualization toggle](armature-visualization-toggle.md)
- [GLTF bone visualization tracking](gltf-bone-viz-tracking-and-toggle.md)

